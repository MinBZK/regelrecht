//! De stroomdefinitie: welke feiten de cel vastlegt, en het bouwen van een
//! gram uit wat binnenkomt.
//!
//! Een veld van een event bindt aan `$intake.<pad>` (wie en langs welke weg:
//! het ontvangstkanaal) of aan `$external.<pad>` (de inhoud zoals ingediend),
//! of is een constante van de stroom. Velden mogen genest zijn, en een
//! `$external`-waarde mag meer dan een veld voeden. Een tabelveld
//! (`{tabel: $external.<pad>, kolommen: [...]}`) declareert zijn kolommen.
//!
//! Wat een indiening onder `external` meegeeft, moet passen in de vorm die de
//! stroom declareert ([`Vorm`]): een onbekend veld of een onbekende kolom
//! wordt geweigerd, met het veldpad.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::schema::{self, Soort};

/// Een geladen stroomdefinitie.
#[derive(Debug, Clone)]
pub struct Stroom {
    pub id: String,
    pub recording_actor: String,
    pub chronicle: String,
    pub events: Vec<Event>,
    /// SHA-256 van het bestand zoals gelezen; gaat mee in elk gram.
    pub sha256: String,
    /// Het document zelf, voor `GET /api/stroom`.
    pub document: Value,
}

/// Een event uit een stroom.
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub name: String,
    pub intake: String,
    pub grondslag: Vec<String>,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub soort: Option<String>,
    /// Bij een stage-decretogram: de stage van het besluit (RFC-008).
    #[serde(default)]
    pub stage: Option<String>,
    /// Of het gram een zaak opent, een bestaande zaak volgt, of geen zaak
    /// heeft. Bepaalt of het gram een `zaakkenmerk` draagt.
    #[serde(default)]
    pub zaak: Zaak,
    /// De veldboom, in documentvolgorde (een YAML-mapping houdt die vast).
    pub fields: serde_yaml_ng::Mapping,
    #[serde(default)]
    pub niet_gereduceerd: Vec<NietGereduceerd>,
}

/// De zaak van een event. Een zaakkenmerk groepeert grammen van een zaak
/// (RFC-022 par. 1.2 doet dat alleen voor de stage-decretogrammen van een
/// besluit); niet elk feit hoort bij een zaak, dus de stroom zegt het per
/// event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Zaak {
    /// De cel geeft bij vastleggen een nieuw zaakkenmerk.
    Opent,
    /// Het gram draagt het zaakkenmerk van een bestaande zaak, dat in de
    /// invoer wordt meegegeven.
    Volgt,
    /// Geen zaakkenmerk.
    #[default]
    Geen,
}

impl Zaak {
    /// Of een gram van dit event een zaakkenmerk draagt.
    pub fn heeft_kenmerk(self) -> bool {
        self != Zaak::Geen
    }

    pub fn als_tekst(self) -> &'static str {
        match self {
            Zaak::Opent => "opent",
            Zaak::Volgt => "volgt",
            Zaak::Geen => "geen",
        }
    }

    /// Of een zaakkenmerk, of het ontbreken ervan, past bij deze zaak.
    pub fn toets_kenmerk(self, event: &str, zaakkenmerk: Option<&str>) -> Result<(), String> {
        match (self.heeft_kenmerk(), zaakkenmerk) {
            (true, None) => Err(format!(
                "event '{event}' heeft zaak: {}, maar het zaakkenmerk ontbreekt",
                self.als_tekst()
            )),
            (false, Some(_)) => Err(format!(
                "event '{event}' heeft geen zaak (zaak: geen) en krijgt geen zaakkenmerk"
            )),
            _ => Ok(()),
        }
    }
}

/// Een veld dat bewust door geen afleiding gelezen wordt, met de reden.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NietGereduceerd {
    pub veld: String,
    pub reden: String,
}

/// Waar de waarde van een veld vandaan komt.
#[derive(Debug, Clone, PartialEq)]
pub enum Binding {
    /// `$intake.<pad>`: het ontvangstkanaal.
    Intake(String),
    /// `$external.<pad>`: de inhoud zoals ingediend.
    External(String),
    /// `{tabel: $external.<pad>, kolommen: [...]}`: een lijst van regels
    /// met de gedeclareerde kolommen.
    Tabel { bron: String, kolommen: Vec<String> },
    /// Een vaste waarde van de stroom.
    Constante(Value),
}

/// De vorm die een indiening onder `external` mag hebben, afgeleid uit de
/// `$external`-bindingen van een event.
#[derive(Debug, Clone, PartialEq)]
pub enum Vorm {
    /// Een enkele waarde (tekst, getal, ja/nee of null).
    Waarde,
    /// Een lijst van regels met alleen deze kolommen.
    Tabel(Vec<String>),
    /// Een object met alleen deze sleutels.
    Tak(BTreeMap<String, Vorm>),
}

/// Een blad van de veldboom: het pad in het gram en waaraan het bindt.
#[derive(Debug, Clone, PartialEq)]
pub struct Blad {
    /// Pad in het gram onder `fields`, bijvoorbeeld `inhoud.organen`.
    pub pad: String,
    pub binding: Binding,
}

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
        let canoniek = serde_json::to_string(
            &serde_json::json!({"regelingen": regelingen, "stromen": stromen}),
        )
        .unwrap_or_default();
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

    /// Valideer het gram tegen `gram.json`.
    pub fn valideer(&self) -> Result<(), Vec<String>> {
        schema::valideer(Soort::Gram, &self.als_json())
    }

    /// De waarde op een pad onder `fields`.
    pub fn veld(&self, pad: &str) -> Option<&Value> {
        let mut delen = pad.split('.');
        let mut huidig = self.fields.get(delen.next()?)?;
        for deel in delen {
            huidig = huidig.as_object()?.get(deel)?;
        }
        Some(huidig)
    }
}

/// Lees een stroomdefinitie uit tekst. `bron` noemt het bestand in meldingen.
pub fn parse(tekst: &str, bron: &str) -> Result<Stroom, Vec<String>> {
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(tekst)
        .map_err(|e| vec![format!("{bron}: geen geldige YAML: {e}")])?;
    let document: Value = serde_json::to_value(&yaml).map_err(|e| vec![format!("{bron}: {e}")])?;
    schema::valideer(Soort::Stroom, &document).map_err(|fouten| {
        fouten
            .into_iter()
            .map(|f| format!("{bron}: {f}"))
            .collect::<Vec<_>>()
    })?;

    #[derive(Deserialize)]
    struct Ruw {
        #[serde(rename = "$id")]
        id: String,
        recording_actor: String,
        chronicle: String,
        events: Vec<Event>,
    }
    let ruw: Ruw = serde_yaml_ng::from_value(yaml).map_err(|e| vec![format!("{bron}: {e}")])?;
    let fouten: Vec<String> = ruw
        .events
        .iter()
        .filter_map(|e| e.external_vorm().err())
        .flatten()
        .map(|f| format!("{bron}: {f}"))
        .collect();
    if !fouten.is_empty() {
        return Err(fouten);
    }
    Ok(Stroom {
        id: ruw.id,
        recording_actor: ruw.recording_actor,
        chronicle: ruw.chronicle,
        events: ruw.events,
        sha256: hex::encode(Sha256::digest(tekst.as_bytes())),
        document,
    })
}

/// Laad de stroomdefinities uit een bestand of uit alle `.yaml`-bestanden in
/// een map.
pub fn laad(pad: &Path) -> Result<Vec<Stroom>, Vec<String>> {
    let bestanden: Vec<std::path::PathBuf> = if pad.is_dir() {
        let mut v: Vec<_> = std::fs::read_dir(pad)
            .map_err(|e| vec![format!("{}: {e}", pad.display())])?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "yaml" || x == "yml"))
            .collect();
        v.sort();
        v
    } else {
        vec![pad.to_path_buf()]
    };
    let mut strommen = Vec::new();
    let mut fouten = Vec::new();
    for bestand in &bestanden {
        let bron = bestand.display().to_string();
        match std::fs::read_to_string(bestand) {
            Ok(tekst) => match parse(&tekst, &bron) {
                Ok(s) => strommen.push(s),
                Err(f) => fouten.extend(f),
            },
            Err(e) => fouten.push(format!("{bron}: {e}")),
        }
    }
    if strommen.is_empty() && fouten.is_empty() {
        fouten.push(format!("{}: geen stroomdefinitie gevonden", pad.display()));
    }
    if fouten.is_empty() {
        Ok(strommen)
    } else {
        Err(fouten)
    }
}

impl Stroom {
    /// Het event met deze naam.
    pub fn event(&self, naam: &str) -> Option<&Event> {
        self.events.iter().find(|e| e.name == naam)
    }
}

impl Event {
    /// De bladeren van de veldboom, in documentvolgorde.
    pub fn bladeren(&self) -> Vec<Blad> {
        use serde_yaml_ng::Value as Y;
        fn loop_(prefix: &str, velden: &serde_yaml_ng::Mapping, uit: &mut Vec<Blad>) {
            for (naam, waarde) in velden {
                let Some(naam) = naam.as_str() else { continue };
                let pad = if prefix.is_empty() {
                    naam.to_string()
                } else {
                    format!("{prefix}.{naam}")
                };
                let binding = match waarde {
                    // Het schema laat `kolommen` als lijst alleen toe in een
                    // tabelveld; een groep velden heeft geen lijsten.
                    Y::Mapping(kind) if kind.get("kolommen").is_some_and(Y::is_sequence) => {
                        let bron = kind
                            .get("tabel")
                            .and_then(Y::as_str)
                            .and_then(|t| t.strip_prefix("$external."))
                            .unwrap_or_default()
                            .to_string();
                        let kolommen = kind
                            .get("kolommen")
                            .and_then(Y::as_sequence)
                            .into_iter()
                            .flatten()
                            .filter_map(Y::as_str)
                            .map(str::to_string)
                            .collect();
                        Binding::Tabel { bron, kolommen }
                    }
                    Y::Mapping(kind) => {
                        loop_(&pad, kind, uit);
                        continue;
                    }
                    Y::String(tekst) => {
                        if let Some(r) = tekst.strip_prefix("$intake.") {
                            Binding::Intake(r.to_string())
                        } else if let Some(r) = tekst.strip_prefix("$external.") {
                            Binding::External(r.to_string())
                        } else {
                            Binding::Constante(Value::String(tekst.clone()))
                        }
                    }
                    ander => Binding::Constante(serde_json::to_value(ander).unwrap_or(Value::Null)),
                };
                uit.push(Blad { pad, binding });
            }
        }
        let mut uit = Vec::new();
        loop_("", &self.fields, &mut uit);
        uit
    }

    /// De velden van een gram van dit event als YAML, in de volgorde van de
    /// stroom (een JSON-object uit de kroniek is alfabetisch).
    pub fn geordend(&self, fields: &Map<String, Value>) -> serde_yaml_ng::Mapping {
        fn loop_(
            sjabloon: &serde_yaml_ng::Mapping,
            waarden: &Map<String, Value>,
        ) -> serde_yaml_ng::Mapping {
            let mut uit = serde_yaml_ng::Mapping::new();
            for (naam, sub) in sjabloon {
                let Some(naam) = naam.as_str() else { continue };
                let Some(waarde) = waarden.get(naam) else {
                    continue;
                };
                let geordend = match (sub, waarde) {
                    (serde_yaml_ng::Value::Mapping(s), Value::Object(w)) => {
                        serde_yaml_ng::Value::Mapping(loop_(s, w))
                    }
                    _ => serde_yaml_ng::to_value(waarde).unwrap_or(serde_yaml_ng::Value::Null),
                };
                uit.insert(serde_yaml_ng::Value::String(naam.to_string()), geordend);
            }
            uit
        }
        loop_(&self.fields, fields)
    }

    /// Of een pad een blad of een tak van de veldboom is.
    pub fn heeft_pad(&self, pad: &str) -> bool {
        self.bladeren()
            .iter()
            .any(|b| b.pad == pad || b.pad.starts_with(&format!("{pad}.")))
    }

    /// Of een pad een blad is (een veld met een eigen waarde, zoals een tabel).
    pub fn heeft_blad(&self, pad: &str) -> bool {
        self.bladeren().iter().any(|b| b.pad == pad)
    }

    /// De kolommen van een tabelveld, of `None` als het pad geen tabelveld is.
    pub fn kolommen(&self, pad: &str) -> Option<Vec<String>> {
        self.bladeren().into_iter().find_map(|b| match b.binding {
            Binding::Tabel { kolommen, .. } if b.pad == pad => Some(kolommen),
            _ => None,
        })
    }

    /// De `$external`-bindingen: het bronpad en de vorm van de waarde.
    fn external_bronnen(&self) -> Vec<(String, Vorm)> {
        self.bladeren()
            .into_iter()
            .filter_map(|b| match b.binding {
                Binding::External(bron) => Some((bron, Vorm::Waarde)),
                Binding::Tabel { bron, kolommen } => Some((bron, Vorm::Tabel(kolommen))),
                _ => None,
            })
            .collect()
    }

    /// De namen die een indiening onder `external` mag meegeven: het eerste
    /// deel van elk `$external`-pad, in de volgorde van de stroom.
    pub fn external_sleutels(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for (bron, _) in self.external_bronnen() {
            let kop = bron.split('.').next().unwrap_or_default().to_string();
            if !v.contains(&kop) {
                v.push(kop);
            }
        }
        v
    }

    /// De vorm die `external` mag hebben. Een fout als twee bindingen
    /// hetzelfde bronpad een andere vorm geven, zoals een enkele waarde en
    /// een tabel, of een waarde en een object met velden eronder.
    pub fn external_vorm(&self) -> Result<BTreeMap<String, Vorm>, Vec<String>> {
        let mut wortel = BTreeMap::new();
        let mut fouten = Vec::new();
        for (bron, vorm) in self.external_bronnen() {
            let delen: Vec<&str> = bron.split('.').collect();
            if !voeg_vorm_toe(&mut wortel, &delen, vorm) {
                fouten.push(format!(
                    "event '{}': '$external.{bron}' krijgt meer dan een vorm (waarde, tabel of velden eronder)",
                    self.name
                ));
            }
        }
        if fouten.is_empty() {
            Ok(wortel)
        } else {
            Err(fouten)
        }
    }
}

/// Zet een vorm op een bronpad. Onwaar als er al een andere vorm staat.
fn voeg_vorm_toe(tak: &mut BTreeMap<String, Vorm>, delen: &[&str], vorm: Vorm) -> bool {
    match delen {
        [] => true,
        [laatste] => {
            let bestaand = tak.entry((*laatste).to_string()).or_insert(vorm.clone());
            *bestaand == vorm
        }
        [kop, rest @ ..] => match tak
            .entry((*kop).to_string())
            .or_insert_with(|| Vorm::Tak(BTreeMap::new()))
        {
            Vorm::Tak(sub) => voeg_vorm_toe(sub, rest, vorm),
            _ => false,
        },
    }
}

/// Een enkele waarde: geen lijst en geen object.
fn enkel(w: &Value) -> bool {
    !matches!(w, Value::Array(_) | Value::Object(_))
}

/// Toets `external` aan de vorm van de stroom. Levert de veldpaden die de
/// stroom niet kent, en de meldingen over waarden van de verkeerde vorm.
fn toets_vorm(
    velden: &Map<String, Value>,
    vorm: &BTreeMap<String, Vorm>,
    prefix: &str,
    onbekend: &mut Vec<String>,
    fouten: &mut Vec<String>,
) {
    for (naam, waarde) in velden {
        let pad = format!("{prefix}{naam}");
        match vorm.get(naam) {
            None => onbekend.push(pad),
            Some(Vorm::Waarde) => {
                if !enkel(waarde) {
                    fouten.push(format!("veld '{pad}' verwacht een enkele waarde"));
                }
            }
            Some(Vorm::Tak(sub)) => match waarde {
                Value::Null => {}
                Value::Object(m) => toets_vorm(m, sub, &format!("{pad}."), onbekend, fouten),
                _ => fouten.push(format!("veld '{pad}' verwacht velden eronder")),
            },
            Some(Vorm::Tabel(kolommen)) => match waarde {
                Value::Null => {}
                Value::Array(regels) => {
                    for (i, regel) in regels.iter().enumerate() {
                        let Value::Object(regel) = regel else {
                            fouten.push(format!("regel '{pad}[{i}]' is geen object met kolommen"));
                            continue;
                        };
                        for (kolom, w) in regel {
                            let kolompad = format!("{pad}[{i}].{kolom}");
                            if !kolommen.contains(kolom) {
                                onbekend.push(kolompad);
                            } else if !enkel(w) {
                                fouten
                                    .push(format!("kolom '{kolompad}' verwacht een enkele waarde"));
                            }
                        }
                    }
                }
                _ => fouten.push(format!(
                    "veld '{pad}' is een tabel en verwacht een lijst van regels"
                )),
            },
        }
    }
}

/// Een tabel zoals het gram hem vastlegt: elke regel met alle gedeclareerde
/// kolommen in de volgorde van de stroom, een ontbrekende kolom als null.
fn als_tabel(waarde: Option<&Value>, kolommen: &[String]) -> Value {
    let Some(Value::Array(regels)) = waarde else {
        return Value::Null;
    };
    Value::Array(
        regels
            .iter()
            .map(|r| {
                Value::Object(
                    kolommen
                        .iter()
                        .map(|k| (k.clone(), r.get(k).cloned().unwrap_or(Value::Null)))
                        .collect(),
                )
            })
            .collect(),
    )
}

/// Wat nodig is om een gram te bouwen, naast de stroom zelf.
pub struct Indiening<'a> {
    /// Het ontvangstkanaal: `kanaal` en wat de login meegeeft.
    pub intake: &'a Value,
    /// De inhoud zoals ingediend.
    pub external: &'a Map<String, Value>,
    pub op_moment: DateTime<FixedOffset>,
    /// Bij `zaak: opent` het nieuwe kenmerk, bij `volgt` dat van de
    /// bestaande zaak, bij `geen` niets.
    pub zaakkenmerk: Option<&'a str>,
}

fn waarde_op<'v>(wortel: &'v Value, pad: &str) -> Option<&'v Value> {
    pad.split('.')
        .try_fold(wortel, |huidig, deel| huidig.as_object()?.get(deel))
}

/// Bouw een gram uit een indiening. Het gram houdt de vorm van de stroom:
/// een veld dat niet is ingevuld staat erin als null, want ook een
/// onvolledige indiening wordt vastgelegd. Een veld of tabelkolom dat de
/// stroom niet kent wordt geweigerd, met het veldpad: wat geen grondslag
/// heeft, wordt niet vastgelegd.
pub fn bouw_gram(
    stroom: &Stroom,
    event: &Event,
    indiening: &Indiening<'_>,
) -> Result<Gram, String> {
    event
        .zaak
        .toets_kenmerk(&event.name, indiening.zaakkenmerk)?;
    let vorm = event.external_vorm().map_err(|f| f.join("; "))?;
    let mut onbekend = Vec::new();
    let mut fouten = Vec::new();
    toets_vorm(indiening.external, &vorm, "", &mut onbekend, &mut fouten);
    if !onbekend.is_empty() {
        onbekend.sort_unstable();
        return Err(format!(
            "onbekend veld {}: het event '{}' legt het niet vast",
            onbekend
                .iter()
                .map(|k| format!("'{k}'"))
                .collect::<Vec<_>>()
                .join(", "),
            event.name
        ));
    }
    if !fouten.is_empty() {
        return Err(fouten.join("; "));
    }

    let external = Value::Object(indiening.external.clone());
    let mut fields = Map::new();
    for blad in event.bladeren() {
        let waarde = match &blad.binding {
            Binding::Intake(bronpad) => {
                waarde_op(indiening.intake, bronpad)
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "het ontvangstkanaal levert '$intake.{bronpad}' niet (veld '{}')",
                            blad.pad
                        )
                    })?
            }
            Binding::External(bronpad) => waarde_op(&external, bronpad)
                .cloned()
                .unwrap_or(Value::Null),
            Binding::Tabel { bron, kolommen } => als_tabel(waarde_op(&external, bron), kolommen),
            Binding::Constante(w) => w.clone(),
        };
        zet(&mut fields, &blad.pad, waarde);
    }

    Ok(Gram {
        kind: "chronolexogram".to_string(),
        type_: event.type_.clone(),
        soort: event.soort.clone(),
        stage: event.stage.clone(),
        name: event.name.clone(),
        chronicle: stroom.chronicle.clone(),
        recording_actor: stroom.recording_actor.clone(),
        grondslag: event.grondslag.clone(),
        legal_character: None,
        decision_type: None,
        regulation: None,
        regulation_valid_from: None,
        competent_authority: None,
        op_moment: indiening
            .op_moment
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        zaak: event.zaak,
        zaakkenmerk: indiening.zaakkenmerk.map(str::to_string),
        stroom: StroomVerwijzing {
            id: stroom.id.clone(),
            sha256: stroom.sha256.clone(),
        },
        herkomst: None,
        fields,
        inputs: BTreeMap::new(),
        receipt: None,
    })
}

fn zet(wortel: &mut Map<String, Value>, pad: &str, waarde: Value) {
    let delen: Vec<&str> = pad.split('.').collect();
    let mut huidig = wortel;
    for deel in &delen[..delen.len() - 1] {
        let volgende = huidig
            .entry((*deel).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !volgende.is_object() {
            *volgende = Value::Object(Map::new());
        }
        let Value::Object(m) = volgende else { return };
        huidig = m;
    }
    if let Some(laatste) = delen.last() {
        huidig.insert((*laatste).to_string(), waarde);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    const STROOM: &str = include_str!("../tests/fixtures/chronicles/test_aanvragen.yaml");

    const ZAAK: &str = "00000000-0000-4000-8000-000000000001";

    fn moment() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2025-03-12T10:14:03+01:00").unwrap()
    }

    fn intake() -> Value {
        json!({"kanaal": "portaal", "eherkenning": {"kvk": "12345678", "persoon": "A. Tester"}})
    }

    #[test]
    fn fixture_laadt_en_heeft_een_hash() {
        let s = parse(STROOM, "fixture").unwrap();
        assert_eq!(s.id, "test_aanvragen");
        assert_eq!(s.sha256.len(), 64);
        assert_eq!(s.events[0].grondslag, vec!["testregeling_aanvraag#1"]);
    }

    #[test]
    fn aanvraag_zonder_vaste_kern_faalt_op_het_schema() {
        let tekst = STROOM.replace("        dagtekening: $external.dagtekening\n", "");
        let fout = parse(&tekst, "t").unwrap_err();
        assert!(
            fout.iter().any(|f| f.contains("/events/0/fields/kern")),
            "{fout:?}"
        );
    }

    #[test]
    fn ongeldige_stroom_faalt_op_het_schema() {
        let fout = parse(
            "$id: x\nrecording_actor: y\nchronicle: z\nevents: []\n",
            "t",
        )
        .unwrap_err();
        assert!(fout[0].contains("/events"), "{fout:?}");
    }

    #[test]
    fn bladeren_in_documentvolgorde() {
        let s = parse(STROOM, "fixture").unwrap();
        let paden: Vec<String> = s.events[0].bladeren().into_iter().map(|b| b.pad).collect();
        assert_eq!(paden[0], "kern.aanvrager.naam");
        assert!(paden.contains(&"inhoud.organen".to_string()));
        assert!(s.events[0].heeft_pad("kern.ondertekend_via"));
        assert!(!s.events[0].heeft_blad("kern.ondertekend_via"));
        assert!(!s.events[0].heeft_pad("inhoud.bestaat_niet"));
        assert_eq!(
            s.events[0].external_sleutels(),
            vec![
                "naam",
                "adres",
                "dagtekening",
                "aanvraagjaar",
                "aanduiding",
                "registratie",
                "organen",
                "rekeningnummer"
            ]
        );
    }

    #[test]
    fn gram_bouwen_uit_intake_en_external() {
        let s = parse(STROOM, "fixture").unwrap();
        let external = json!({"naam": "Vereniging Voorbeeld", "aanvraagjaar": 2025, "organen": [{"orgaan": "raad", "zetels": 2}]});
        let gram = bouw_gram(
            &s,
            &s.events[0],
            &Indiening {
                intake: &intake(),
                external: external.as_object().unwrap(),
                op_moment: moment(),
                zaakkenmerk: Some(ZAAK),
            },
        )
        .unwrap();
        assert_eq!(gram.type_, "indiening");
        assert_eq!(gram.soort.as_deref(), Some("aanvraag"));
        assert_eq!(gram.op_moment, "2025-03-12T10:14:03+01:00");
        assert_eq!(
            gram.veld("kern.ondertekend_via.kvk_nummer"),
            Some(&json!("12345678"))
        );
        // Een $external-waarde voedt twee velden.
        assert_eq!(
            gram.veld("inhoud.naam"),
            Some(&json!("Vereniging Voorbeeld"))
        );
        assert_eq!(
            gram.veld("kern.aanvrager.naam"),
            Some(&json!("Vereniging Voorbeeld"))
        );
        // Een constante van de stroom.
        assert_eq!(
            gram.veld("kern.gevraagde_beschikking"),
            Some(&json!("testbeschikking, testregeling artikel 1"))
        );
        // Niet ingevuld: vastgelegd als null, de vorm blijft.
        assert_eq!(gram.veld("inhoud.aanduiding"), Some(&Value::Null));
        gram.valideer().unwrap();
    }

    #[test]
    fn onbekend_veld_wordt_geweigerd() {
        let s = parse(STROOM, "fixture").unwrap();
        let external = json!({"schoenmaat": 44});
        let fout = bouw_gram(
            &s,
            &s.events[0],
            &Indiening {
                intake: &intake(),
                external: external.as_object().unwrap(),
                op_moment: moment(),
                zaakkenmerk: Some(ZAAK),
            },
        )
        .unwrap_err();
        assert!(fout.contains("'schoenmaat'"), "{fout}");
    }

    fn bouw(s: &Stroom, external: Value) -> Result<Gram, String> {
        bouw_gram(
            s,
            &s.events[0],
            &Indiening {
                intake: &intake(),
                external: external.as_object().unwrap(),
                op_moment: moment(),
                zaakkenmerk: Some(ZAAK),
            },
        )
    }

    #[test]
    fn tabelveld_declareert_zijn_kolommen() {
        let s = parse(STROOM, "fixture").unwrap();
        let e = &s.events[0];
        assert_eq!(
            e.kolommen("inhoud.organen").unwrap(),
            vec!["orgaan", "zetels", "samengevoegd", "aantal_aanduidingen"]
        );
        assert_eq!(e.kolommen("inhoud.naam"), None);
        assert!(e.heeft_blad("inhoud.organen"));
    }

    #[test]
    fn onbekende_kolom_wordt_geweigerd_met_veldpad() {
        let s = parse(STROOM, "fixture").unwrap();
        let fout = bouw(
            &s,
            json!({"organen": [{"orgaan": "raad"}, {"orgaan": "raad", "kleur": "rood"}]}),
        )
        .unwrap_err();
        assert!(fout.contains("onbekend veld 'organen[1].kleur'"), "{fout}");
    }

    #[test]
    fn tabel_krijgt_elke_kolom_in_het_gram() {
        let s = parse(STROOM, "fixture").unwrap();
        let gram = bouw(&s, json!({"organen": [{"zetels": 3, "orgaan": "raad"}]})).unwrap();
        assert_eq!(
            gram.veld("inhoud.organen"),
            Some(
                &json!([{"orgaan": "raad", "zetels": 3, "samengevoegd": null, "aantal_aanduidingen": null}])
            )
        );
        gram.valideer().unwrap();
        // Niet ingevuld: null, net als een gewoon veld.
        let gram = bouw(&s, json!({})).unwrap();
        assert_eq!(gram.veld("inhoud.organen"), Some(&Value::Null));
        gram.valideer().unwrap();
    }

    #[test]
    fn waarde_van_de_verkeerde_vorm_wordt_geweigerd() {
        let s = parse(STROOM, "fixture").unwrap();
        for (external, verwacht) in [
            (json!({"organen": "raad"}), "veld 'organen' is een tabel"),
            (
                json!({"organen": ["raad"]}),
                "regel 'organen[0]' is geen object",
            ),
            (
                json!({"organen": [{"zetels": {"aantal": 3}}]}),
                "kolom 'organen[0].zetels' verwacht een enkele waarde",
            ),
            (
                json!({"naam": {"voornaam": "A"}}),
                "veld 'naam' verwacht een enkele waarde",
            ),
        ] {
            let fout = bouw(&s, external).unwrap_err();
            assert!(fout.contains(verwacht), "{fout}");
        }
    }

    #[test]
    fn onbekend_veld_in_een_genest_external_object() {
        let tekst = STROOM.replace("adres: $external.adres", "adres: $external.adres.straat");
        let s = parse(&tekst, "t").unwrap();
        let gram = bouw(&s, json!({"adres": {"straat": "Voorbeeldstraat 1"}})).unwrap();
        assert_eq!(
            gram.veld("kern.aanvrager.adres"),
            Some(&json!("Voorbeeldstraat 1"))
        );
        let fout = bouw(&s, json!({"adres": {"straat": "x", "huisdier": "kat"}})).unwrap_err();
        assert!(fout.contains("onbekend veld 'adres.huisdier'"), "{fout}");
    }

    #[test]
    fn een_bronpad_met_twee_vormen_faalt_bij_het_laden() {
        let tekst = STROOM.replace(
            "rekeningnummer: $external.rekeningnummer",
            "rekeningnummer: $external.organen",
        );
        let fout = parse(&tekst, "t").unwrap_err();
        assert!(
            fout.iter()
                .any(|f| f.contains("'$external.organen' krijgt meer dan een vorm")),
            "{fout:?}"
        );
    }

    #[test]
    fn tabel_zonder_kolommen_faalt_op_het_schema() {
        let tekst = STROOM.replace(
            "          kolommen: [orgaan, zetels, samengevoegd, aantal_aanduidingen]\n",
            "          kolommen: []\n",
        );
        let fout = parse(&tekst, "t").unwrap_err();
        assert!(
            fout.iter()
                .any(|f| f.contains("/events/0/fields/inhoud/organen")),
            "{fout:?}"
        );
    }

    #[test]
    fn zaakkenmerk_volgt_de_zaak_van_het_event() {
        for (zaak, kenmerk, fout) in [
            ("opent", Some(ZAAK), None),
            ("volgt", Some(ZAAK), None),
            ("geen", None, None),
            (
                "opent",
                None,
                Some("zaak: opent, maar het zaakkenmerk ontbreekt"),
            ),
            (
                "volgt",
                None,
                Some("zaak: volgt, maar het zaakkenmerk ontbreekt"),
            ),
            ("geen", Some(ZAAK), Some("heeft geen zaak (zaak: geen)")),
        ] {
            let s = parse(
                &STROOM.replace("zaak: opent", &format!("zaak: {zaak}")),
                "t",
            )
            .unwrap();
            let uitkomst = bouw_gram(
                &s,
                &s.events[0],
                &Indiening {
                    intake: &intake(),
                    external: &Map::new(),
                    op_moment: moment(),
                    zaakkenmerk: kenmerk,
                },
            );
            match fout {
                None => {
                    let gram = uitkomst.unwrap();
                    assert_eq!(gram.zaakkenmerk.as_deref(), kenmerk);
                    let json = gram.als_json();
                    // Zonder zaak staat er geen zaak en geen zaakkenmerk in het gram.
                    assert_eq!(json.get("zaak").is_some(), zaak != "geen");
                    assert_eq!(json.get("zaakkenmerk").is_some(), zaak != "geen");
                    gram.valideer().unwrap();
                }
                Some(f) => assert!(uitkomst.unwrap_err().contains(f), "{zaak}"),
            }
        }
        // Zonder `zaak` in de stroom: geen.
        let s = parse(&STROOM.replace("    zaak: opent\n", ""), "t").unwrap();
        assert_eq!(s.events[0].zaak, Zaak::Geen);
    }

    #[test]
    fn ontbrekende_intake_is_een_fout() {
        let s = parse(STROOM, "fixture").unwrap();
        let external = Map::new();
        let fout = bouw_gram(
            &s,
            &s.events[0],
            &Indiening {
                intake: &json!({"kanaal": "portaal"}),
                external: &external,
                op_moment: moment(),
                zaakkenmerk: Some(ZAAK),
            },
        )
        .unwrap_err();
        assert!(fout.contains("$intake.eherkenning.kvk"), "{fout}");
    }
}
