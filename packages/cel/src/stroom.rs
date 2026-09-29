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

use crate::datum;
use crate::gram::{op_pad, zet_pad, Gram, StroomVerwijzing};
use crate::laden;
use crate::schema::Soort;

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
    /// De artikelen die dit event vestigen (`produces.extensions.chronolex`
    /// in de wet, zie [`crate::wet`]). Dan komen `type`, `soort`, `stage`,
    /// `verwijst`, `grondslag` en de grondslag van `op_moment` uit de wet; de
    /// runtime vult ze in bij het laden van de cel, voordat iets anders het
    /// event leest.
    #[serde(default)]
    pub establishes: Vec<String>,
    #[serde(default)]
    pub legal_basis: Vec<String>,
    #[serde(rename = "type", default)]
    pub type_: String,
    #[serde(default)]
    pub subtype: Option<String>,
    /// Bij een stage-decretogram: de stage van het besluit (RFC-008).
    #[serde(default)]
    pub stage: Option<String>,
    /// Naar welke grammen een gram van dit event verwijst, per naam uit de
    /// wettekst, en wat elk mag aanwijzen (zie [`Verwijzing`]).
    #[serde(default)]
    pub refers_to: BTreeMap<String, Verwijzing>,
    /// Afgeleid bij het laden van de cel (zie [`leid_rollen_af`]): of een
    /// gram van dit event een groep opent (een wortel waar andere naar
    /// verwijzen), bij een groep hoort, of los staat. Niet in de YAML.
    #[serde(skip)]
    pub case: Zaak,
    /// Afgeleid bij het laden van de cel (zie [`leid_rollen_af`]): of een
    /// gram van dit event een besluit is, een besluit volgt of een besluit
    /// wijzigt. Niet in de YAML.
    #[serde(skip)]
    pub decision: Option<Decision>,
    /// Waaraan het `op_moment` van het gram bindt, als dat niet het moment
    /// van vastleggen is.
    #[serde(default)]
    pub effective_at: Option<OpMomentBinding>,
    /// De veldboom, in documentvolgorde (een YAML-mapping houdt die vast).
    pub fields: serde_yaml_ng::Mapping,
    #[serde(default)]
    pub not_reduced: Vec<NietGereduceerd>,
    /// De naamsbrug uit de wet (zie [`crate::wet::Vestiging::als`]):
    /// `<naam bij de lezer>: <veld van dit event>`.
    #[serde(skip)]
    pub aliases: BTreeMap<String, String>,
    /// Het type van een veld zoals de wet het noemt (`velden: {bedrag: {type:
    /// amount, unit: eurocent}}` in `vestigt`): het formulier van een feit
    /// neemt het over als geen lezing het veld leest.
    #[serde(skip)]
    pub field_types: BTreeMap<String, crate::wet::Veldtype>,
}

/// Een verwijzing van een event: wat het gram waarnaar een gram van dit
/// event verwijst moet zijn (`naar`), en of het proces haar moet meegeven.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Verwijzing {
    pub to: Naar,
    #[serde(default)]
    pub required: bool,
}

/// Wat een gram moet zijn om het doel van een verwijzing te zijn.
#[derive(Debug, Clone, PartialEq)]
pub enum Naar {
    /// Gevestigd door dit artikel (`<regeling>#<artikel>`): het event van het
    /// doel noemt het in `vestigt`, of het gram draagt het als grondslag.
    Artikel(String),
    /// Een gram van dit event.
    Event(String),
    /// Een gram met deze stage.
    Stage(String),
}

impl<'de> Deserialize<'de> for Naar {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Ruw {
            Tekst(String),
            Stage { stage: String },
        }
        Ok(match Ruw::deserialize(d)? {
            Ruw::Tekst(t) if t.contains('#') => Naar::Artikel(t),
            Ruw::Tekst(t) => Naar::Event(t),
            Ruw::Stage { stage } => Naar::Stage(stage),
        })
    }
}

impl std::fmt::Display for Naar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Naar::Artikel(a) => write!(f, "gevestigd door {a}"),
            Naar::Event(e) => write!(f, "een gram '{e}'"),
            Naar::Stage(s) => write!(f, "een gram met stage {s}"),
        }
    }
}

impl Naar {
    /// Of een gram van `event` (in de cel met deze strommen) het doel kan
    /// zijn. Bij een artikel: het event noemt het in `vestigt`, of zijn
    /// grondslag begint ermee.
    pub fn past_event(&self, event: &Event) -> bool {
        match self {
            Naar::Artikel(a) => {
                event.establishes.contains(a)
                    || event
                        .legal_basis
                        .iter()
                        .any(|g| g == a || g.starts_with(&format!("{a} ")))
            }
            Naar::Event(e) => event.name == *e,
            Naar::Stage(s) => event.stage.as_deref() == Some(s),
        }
    }

    /// Of `gram` het doel kan zijn; `event` is het event van het gram, als
    /// de cel het kent.
    pub fn past(&self, gram: &Gram, event: Option<&Event>) -> bool {
        match self {
            Naar::Artikel(a) => {
                event.is_some_and(|e| e.establishes.contains(a))
                    || gram
                        .legal_basis
                        .iter()
                        .any(|g| g == a || g.starts_with(&format!("{a} ")))
            }
            Naar::Event(e) => gram.name == *e,
            Naar::Stage(s) => gram.stage.as_deref() == Some(s),
        }
    }
}

/// De rol van een event tegenover een groep, afgeleid uit de verwijzingen
/// (zie [`leid_rollen_af`]). Er is geen zaak in het gram: dit zegt alleen of
/// een gram van het event een wortel is waar andere grammen naar verwijzen
/// (`Opent`, zoals de aanvraag), naar een ander gram verwijst (`Volgt`), of
/// los staat (`Geen`, zoals een registerfeit dat niemand volgt).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Zaak {
    /// Een gram van dit event is een wortel waar andere events naar verwijzen.
    Opens,
    /// Een gram van dit event verwijst naar een ander gram.
    Follows,
    /// Geen van beide.
    #[default]
    Standalone,
}

impl Zaak {
    /// Of een gram van dit event bij een groep hoort.
    pub fn heeft_kenmerk(self) -> bool {
        self != Zaak::Standalone
    }

    pub fn als_tekst(self) -> &'static str {
        match self {
            Zaak::Opens => "opens",
            Zaak::Follows => "follows",
            Zaak::Standalone => "standalone",
        }
    }
}

/// De rol van een event tegenover een besluit, afgeleid uit stage en
/// verwijzingen (zie [`leid_rollen_af`]). Een besluit is de state container
/// van RFC-008; de stage-grammen van een besluit verwijzen ernaar (RFC-022
/// par. 1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// Het gram is een besluit (stage BESLUIT, zonder `wijzigt`). Een tweede
    /// besluit van hetzelfde event dat naar hetzelfde gram verwijst, weigert
    /// de cel: een ander besluit hierover vraagt een eigen grondslag.
    Opens,
    /// Het gram verwijst naar een besluit, zoals de bekendmaking of een
    /// betaling die het uitvoert (RFC-022 par. 3.3 `references_decision`).
    Follows,
    /// Het gram is een besluit dat met `wijzigt` naar een ander besluit
    /// verwijst (RFC-022 par. 3.1, Awb 4:48 en 4:49).
    Amends,
}

impl Decision {
    pub fn als_tekst(self) -> &'static str {
        match self {
            Decision::Opens => "opens",
            Decision::Follows => "follows",
            Decision::Amends => "amends",
        }
    }

    /// Of een gram van dit event zelf een besluit is.
    pub fn is_besluit(self) -> bool {
        self != Decision::Follows
    }
}

/// De stage van een besluit (RFC-008).
pub const BESLUIT: &str = "BESLUIT";

/// De naam van de verwijzing waarmee een besluit een ander besluit wijzigt.
pub const WIJZIGT: &str = "amends";

/// Leid per event van een cel de rollen af (zie [`Zaak`] en [`Besluit`]),
/// uit de stages en de verwijzingen van alle events van de cel. Een event
/// met stage BESLUIT is een besluit, met een verwijzing `wijzigt` een
/// wijziging; een ander event volgt een besluit als een van zijn
/// verwijzingen alleen op een besluit kan wijzen. Een event verwijst naar
/// een groep als het verwijzingen heeft, en opent er een als een ander event
/// naar een gram van dit event kan verwijzen en het zelf nergens naar
/// verwijst.
pub fn leid_rollen_af(streams: &mut [Stroom]) {
    let events: Vec<Event> = streams.iter().flat_map(|s| s.events.clone()).collect();
    let is_besluit = |e: &Event| e.stage.as_deref() == Some(BESLUIT);
    for s in streams.iter_mut() {
        for e in &mut s.events {
            e.decision = if is_besluit(e) {
                Some(if e.refers_to.contains_key(WIJZIGT) {
                    Decision::Amends
                } else {
                    Decision::Opens
                })
            } else if e.refers_to.values().any(|v| {
                let doelen: Vec<&Event> = events.iter().filter(|d| v.to.past_event(d)).collect();
                !doelen.is_empty() && doelen.iter().all(|d| is_besluit(d))
            }) {
                Some(Decision::Follows)
            } else {
                None
            };
            let wordt_gevolgd = events
                .iter()
                .any(|a| a.refers_to.values().any(|v| v.to.past_event(e)));
            e.case = if !e.refers_to.is_empty() {
                Zaak::Follows
            } else if wordt_gevolgd {
                Zaak::Opens
            } else {
                Zaak::Standalone
            };
        }
    }
}

/// Controleer de verwijzingen van de events van een cel: elke verwijzing kan
/// naar een event van de cel wijzen.
pub fn controleer_verwijzingen(streams: &[Stroom]) -> Vec<String> {
    let events: Vec<&Event> = streams.iter().flat_map(|s| s.events.iter()).collect();
    let mut fouten = Vec::new();
    for s in streams {
        for e in &s.events {
            for (name, v) in &e.refers_to {
                if !events.iter().any(|d| v.to.past_event(d)) {
                    fouten.push(format!(
                        "stroom '{}', event '{}': verwijzing '{name}' wijst naar {}, maar geen event van de cel past",
                        s.id, e.name, v.to
                    ));
                }
            }
        }
    }
    fouten
}

/// Wat een sleutel van het gram zelf bij een event is (zie [`Event::kenmerk`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eventkenmerk<'a> {
    /// Vast in de stroom; `None` als een gram van dit event het niet heeft.
    Vast(Option<&'a str>),
    /// Hangt van het gram af, zoals het id, de wortel of een verwijzing.
    Vrij,
    /// Een gram van dit event heeft het nooit.
    Nooit,
}

/// Het `op_moment` van een event gebonden aan een ingediende waarde, met de
/// grondslag waarom dat moment rechtens telt. Voorbeeld: de dag waarop een
/// papieren aanvraag binnenkwam (Awb 4:13: de termijn loopt vanaf de
/// ontvangst), vastgelegd op een latere dag.
#[derive(Debug, Clone, Deserialize)]
pub struct OpMomentBinding {
    /// `$intake.<pad>` of `$external.<pad>`.
    pub source: String,
    /// Uit de wet als het event `vestigt` heeft.
    #[serde(default)]
    pub legal_basis: Vec<String>,
}

impl OpMomentBinding {
    /// De bron als [`Binding`]: het schema laat alleen `$intake` en
    /// `$external` toe.
    pub fn binding(&self) -> Binding {
        match self.source.strip_prefix("$intake.") {
            Some(r) => Binding::Intake(r.to_string()),
            None => Binding::External(
                self.source
                    .strip_prefix("$external.")
                    .unwrap_or(&self.source)
                    .to_string(),
            ),
        }
    }
}

/// Een veld dat bewust door geen afleiding gelezen wordt, met de reden.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NietGereduceerd {
    pub field: String,
    pub reason: String,
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
    Tabel {
        source: String,
        columns: Vec<String>,
    },
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
    pub path: String,
    pub binding: Binding,
}

/// Lees een stroomdefinitie uit tekst. `bron` noemt het bestand in meldingen.
pub fn parse(tekst: &str, source: &str) -> Result<Stroom, Vec<String>> {
    // De YAML-boom houdt de volgorde van de velden vast; zie `Event::fields`.
    let (yaml, document) = laden::yaml_document(tekst, source, Soort::Stroom)?;

    #[derive(Deserialize)]
    struct Ruw {
        #[serde(rename = "$id")]
        id: String,
        recording_actor: String,
        chronicle: String,
        events: Vec<Event>,
    }
    let ruw: Ruw = serde_yaml_ng::from_value(yaml).map_err(|e| vec![format!("{source}: {e}")])?;
    let fouten: Vec<String> = ruw
        .events
        .iter()
        .filter_map(|e| e.external_vorm().err())
        .flatten()
        .map(|f| format!("{source}: {f}"))
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
pub fn laad(path: &Path) -> Result<Vec<Stroom>, Vec<String>> {
    let bestanden: Vec<std::path::PathBuf> = if path.is_dir() {
        laden::yaml_bestanden(path).map_err(|e| vec![e])?
    } else {
        vec![path.to_path_buf()]
    };
    let mut streams = Vec::new();
    let mut fouten = Vec::new();
    for bestand in &bestanden {
        match laden::laad(bestand, parse) {
            Ok(s) => streams.push(s),
            Err(f) => fouten.extend(f),
        }
    }
    if streams.is_empty() && fouten.is_empty() {
        fouten.push(format!("{}: geen stroomdefinitie gevonden", path.display()));
    }
    if fouten.is_empty() {
        Ok(streams)
    } else {
        Err(fouten)
    }
}

impl Stroom {
    /// Het event met deze naam.
    pub fn event(&self, name: &str) -> Option<&Event> {
        self.events.iter().find(|e| e.name == name)
    }
}

impl Event {
    /// De bladeren van de veldboom, in documentvolgorde.
    pub fn bladeren(&self) -> Vec<Blad> {
        use serde_yaml_ng::Value as Y;
        fn loop_(prefix: &str, fields: &serde_yaml_ng::Mapping, uit: &mut Vec<Blad>) {
            for (name, value) in fields {
                let Some(name) = name.as_str() else { continue };
                let path = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{prefix}.{name}")
                };
                let binding = match value {
                    // Het schema laat `kolommen` als lijst alleen toe in een
                    // tabelveld; een groep velden heeft geen lijsten.
                    Y::Mapping(kind) if kind.get("columns").is_some_and(Y::is_sequence) => {
                        let source = kind
                            .get("table")
                            .and_then(Y::as_str)
                            .and_then(|t| t.strip_prefix("$external."))
                            .unwrap_or_default()
                            .to_string();
                        let columns = kind
                            .get("columns")
                            .and_then(Y::as_sequence)
                            .into_iter()
                            .flatten()
                            .filter_map(Y::as_str)
                            .map(str::to_string)
                            .collect();
                        Binding::Tabel { source, columns }
                    }
                    Y::Mapping(kind) => {
                        loop_(&path, kind, uit);
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
                uit.push(Blad { path, binding });
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
            for (name, sub) in sjabloon {
                let Some(name) = name.as_str() else { continue };
                let Some(value) = waarden.get(name) else {
                    continue;
                };
                let geordend = match (sub, value) {
                    (serde_yaml_ng::Value::Mapping(s), Value::Object(w)) => {
                        serde_yaml_ng::Value::Mapping(loop_(s, w))
                    }
                    _ => serde_yaml_ng::to_value(value).unwrap_or(serde_yaml_ng::Value::Null),
                };
                uit.insert(serde_yaml_ng::Value::String(name.to_string()), geordend);
            }
            uit
        }
        loop_(&self.fields, fields)
    }

    /// Wat een filtersleutel van het gram zelf (zie
    /// [`crate::reductie::GRAM_SLEUTELS`]) bij een gram van dit event kan
    /// zijn, in stroom `stroom`. `None` als `sleutel` een veldpad is. Zo
    /// lezen de controle bij het opstarten en de reductie ([`crate::gram::Gram::kenmerk`])
    /// dezelfde sleutels.
    pub fn kenmerk<'a>(&'a self, stream: &'a Stroom, sleutel: &str) -> Option<Eventkenmerk<'a>> {
        let vrij_als = |kan: bool| {
            if kan {
                Eventkenmerk::Vrij
            } else {
                Eventkenmerk::Nooit
            }
        };
        if let Some(name) = sleutel.strip_prefix(crate::gram::VERWIJST) {
            return Some(vrij_als(self.refers_to.contains_key(name)));
        }
        Some(match sleutel {
            "id" | "root" => Eventkenmerk::Vrij,
            "name" => Eventkenmerk::Vast(Some(self.name.as_str())),
            "type" => Eventkenmerk::Vast(Some(self.type_.as_str())),
            "subtype" => Eventkenmerk::Vast(self.subtype.as_deref()),
            "stage" => Eventkenmerk::Vast(self.stage.as_deref()),
            "recording_actor" => Eventkenmerk::Vast(Some(stream.recording_actor.as_str())),
            "chronicle" => Eventkenmerk::Vast(Some(stream.chronicle.as_str())),
            // De velden van een besluit die een proces meegeeft.
            "legal_character" | "decision_type" | "regulation" | "competent_authority" => {
                vrij_als(self.type_ == "decretogram")
            }
            _ => return None,
        })
    }

    /// Of een pad een blad of een tak van de veldboom is.
    pub fn heeft_pad(&self, path: &str) -> bool {
        self.bladeren()
            .iter()
            .any(|b| b.path == path || b.path.starts_with(&format!("{path}.")))
    }

    /// Of een pad een blad is (een veld met een eigen waarde, zoals een tabel).
    pub fn heeft_blad(&self, path: &str) -> bool {
        self.bladeren().iter().any(|b| b.path == path)
    }

    /// De kolommen van een tabelveld, of `None` als het pad geen tabelveld is.
    pub fn columns(&self, path: &str) -> Option<Vec<String>> {
        self.bladeren().into_iter().find_map(|b| match b.binding {
            Binding::Tabel { columns, .. } if b.path == path => Some(columns),
            _ => None,
        })
    }

    /// De `$external`-bindingen: het bronpad en de vorm van de waarde, ook
    /// die van `op_moment`.
    fn external_bronnen(&self) -> Vec<(String, Vorm)> {
        self.bladeren()
            .into_iter()
            .map(|b| b.binding)
            .chain(self.effective_at.as_ref().map(OpMomentBinding::binding))
            .filter_map(|b| match b {
                Binding::External(source) => Some((source, Vorm::Waarde)),
                Binding::Tabel { source, columns } => Some((source, Vorm::Tabel(columns))),
                _ => None,
            })
            .collect()
    }

    /// De namen die een indiening onder `external` mag meegeven: het eerste
    /// deel van elk `$external`-pad, in de volgorde van de stroom.
    pub fn external_sleutels(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for (source, _) in self.external_bronnen() {
            let kop = source.split('.').next().unwrap_or_default().to_string();
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
        let mut root = BTreeMap::new();
        let mut fouten = Vec::new();
        for (source, vorm) in self.external_bronnen() {
            let delen: Vec<&str> = source.split('.').collect();
            if !voeg_vorm_toe(&mut root, &delen, vorm) {
                fouten.push(format!(
                    "event '{}': '$external.{source}' krijgt meer dan een vorm (waarde, tabel of velden eronder)",
                    self.name
                ));
            }
        }
        if fouten.is_empty() {
            Ok(root)
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
    fields: &Map<String, Value>,
    vorm: &BTreeMap<String, Vorm>,
    prefix: &str,
    onbekend: &mut Vec<String>,
    fouten: &mut Vec<String>,
) {
    for (name, value) in fields {
        let path = format!("{prefix}{name}");
        match vorm.get(name) {
            None => onbekend.push(path),
            Some(Vorm::Waarde) => {
                if !enkel(value) {
                    fouten.push(format!("veld '{path}' verwacht een enkele waarde"));
                }
            }
            Some(Vorm::Tak(sub)) => match value {
                Value::Null => {}
                Value::Object(m) => toets_vorm(m, sub, &format!("{path}."), onbekend, fouten),
                _ => fouten.push(format!("veld '{path}' verwacht velden eronder")),
            },
            Some(Vorm::Tabel(columns)) => match value {
                Value::Null => {}
                Value::Array(rows) => {
                    for (i, regel) in rows.iter().enumerate() {
                        let Value::Object(regel) = regel else {
                            fouten.push(format!("regel '{path}[{i}]' is geen object met kolommen"));
                            continue;
                        };
                        for (column, w) in regel {
                            let kolompad = format!("{path}[{i}].{column}");
                            if !columns.contains(column) {
                                onbekend.push(kolompad);
                            } else if !enkel(w) {
                                fouten
                                    .push(format!("kolom '{kolompad}' verwacht een enkele waarde"));
                            }
                        }
                    }
                }
                _ => fouten.push(format!(
                    "veld '{path}' is een tabel en verwacht een lijst van regels"
                )),
            },
        }
    }
}

/// Een tabel zoals het gram hem vastlegt: elke regel met alle gedeclareerde
/// kolommen in de volgorde van de stroom, een ontbrekende kolom als null.
fn als_tabel(value: Option<&Value>, columns: &[String]) -> Value {
    let Some(Value::Array(rows)) = value else {
        return Value::Null;
    };
    Value::Array(
        rows.iter()
            .map(|r| {
                Value::Object(
                    columns
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
    /// Het moment van vastleggen: de klok van de cel.
    pub recorded_at: DateTime<FixedOffset>,
    /// Per verwijzing van het event het id van het gram waarnaar het gram
    /// verwijst. Dat het bestaat en past, toetst de cel onder haar slot.
    pub refers_to: &'a BTreeMap<String, String>,
}

/// Bouw een gram uit een indiening. Het gram houdt de vorm van de stroom:
/// een veld dat niet is ingevuld staat erin als null, want ook een
/// onvolledige indiening wordt vastgelegd. Een veld of tabelkolom dat de
/// stroom niet kent wordt geweigerd, met het veldpad: wat geen grondslag
/// heeft, wordt niet vastgelegd.
pub fn bouw_gram(
    stream: &Stroom,
    event: &Event,
    indiening: &Indiening<'_>,
) -> Result<Gram, String> {
    toets_verwijzingen(event, indiening.refers_to)?;
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

    let mut fields = Map::new();
    for blad in event.bladeren() {
        let value = match &blad.binding {
            Binding::Intake(bronpad) => {
                intake_waarde(indiening, bronpad).cloned().ok_or_else(|| {
                    format!(
                        "het ontvangstkanaal levert '$intake.{bronpad}' niet (veld '{}')",
                        blad.path
                    )
                })?
            }
            Binding::External(bronpad) => op_pad(indiening.external, bronpad)
                .cloned()
                .unwrap_or(Value::Null),
            Binding::Tabel { source, columns } => {
                als_tabel(op_pad(indiening.external, source), columns)
            }
            Binding::Constante(w) => w.clone(),
        };
        zet_pad(&mut fields, &blad.path, value);
    }
    let (effective_at, effective_at_legal_basis) = op_moment_van(event, indiening)?;

    Ok(Gram {
        kind: "chronolexogram".to_string(),
        id: crate::gram::nieuw_id(indiening.recorded_at),
        type_: event.type_.clone(),
        subtype: event.subtype.clone(),
        stage: event.stage.clone(),
        name: event.name.clone(),
        chronicle: stream.chronicle.clone(),
        recording_actor: stream.recording_actor.clone(),
        legal_basis: event.legal_basis.clone(),
        legal_character: None,
        decision_type: None,
        regulation: None,
        regulation_valid_from: None,
        competent_authority: None,
        acting_actor: None,
        effective_at: datum::als_op_moment(&effective_at),
        effective_at_legal_basis,
        recorded_at: datum::als_op_moment(&indiening.recorded_at),
        refers_to: indiening.refers_to.clone(),
        stream: StroomVerwijzing {
            id: stream.id.clone(),
            sha256: stream.sha256.clone(),
        },
        provenance: None,
        fields,
        inputs: BTreeMap::new(),
        receipt: None,
        tijden: Default::default(),
        root: None,
    })
}

/// Of de verwijzingen die het proces meegeeft passen bij het event: elke
/// naam is een verwijzing van het event, en elke verplichte verwijzing is er.
/// Of het gram waarnaar verwezen wordt bestaat en past, toetst de cel onder
/// haar slot.
pub fn toets_verwijzingen(
    event: &Event,
    refers_to: &BTreeMap<String, String>,
) -> Result<(), String> {
    let name = &event.name;
    for n in refers_to.keys() {
        if !event.refers_to.contains_key(n) {
            let kan: Vec<&str> = event.refers_to.keys().map(String::as_str).collect();
            return Err(format!(
                "event '{name}' verwijst niet met '{n}' (wel: {})",
                if kan.is_empty() {
                    "geen verwijzing".to_string()
                } else {
                    kan.join(", ")
                }
            ));
        }
    }
    for (n, v) in &event.refers_to {
        if v.required && !refers_to.contains_key(n) {
            return Err(format!(
                "event '{name}' verwijst verplicht met '{n}' naar {}: geef het id mee",
                v.to
            ));
        }
    }
    Ok(())
}

fn intake_waarde<'i>(indiening: &'i Indiening<'_>, path: &str) -> Option<&'i Value> {
    indiening.intake.as_object().and_then(|i| op_pad(i, path))
}

/// Het moment waaraan het `op_moment` van een event een ingediende waarde
/// bindt (`$intake.<pad>` of `$external.<pad>`), gelezen, met die binding.
/// `None` als het event niets bindt of de waarde er niet is (of null). Een
/// waarde die geen datum of moment is, is een fout; een datum is het begin
/// van die dag in de tijdzone `offset`. Het proces leest zo de peildatum van
/// een handeling uit zijn formulier, en de cel het `op_moment` van een gram.
pub fn gebonden_moment<'e>(
    event: &'e Event,
    intake: Option<&Map<String, Value>>,
    external: &Map<String, Value>,
    offset: FixedOffset,
) -> Result<Option<(DateTime<FixedOffset>, &'e OpMomentBinding)>, String> {
    let Some(b) = &event.effective_at else {
        return Ok(None);
    };
    let value = match b.binding() {
        Binding::Intake(path) => intake.and_then(|i| op_pad(i, &path)),
        Binding::External(path) => op_pad(external, &path),
        _ => None,
    };
    let Some(tekst) = value.filter(|w| !w.is_null()) else {
        return Ok(None);
    };
    let tekst = tekst.as_str().ok_or_else(|| {
        format!(
            "'{}' (op_moment van event '{}') is geen datum of moment",
            b.source, event.name
        )
    })?;
    let moment =
        datum::Tijdpunt::lees(&format!("op_moment uit '{}'", b.source), tekst)?.als_moment(offset);
    Ok(Some((moment, b)))
}

/// Het `op_moment` van een gram en, als het event het aan een ingediende
/// waarde bond en die er was, de grondslag daarvan. Zonder waarde (of zonder
/// binding) is het het moment van vastleggen. Een gebonden moment na het
/// vastleggen wordt geweigerd: wat nog moet gebeuren, is geen feit.
fn op_moment_van(
    event: &Event,
    indiening: &Indiening<'_>,
) -> Result<(DateTime<FixedOffset>, Option<Vec<String>>), String> {
    let nu = indiening.recorded_at;
    let intake = indiening.intake.as_object();
    let Some((moment, b)) = gebonden_moment(event, intake, indiening.external, *nu.offset())?
    else {
        return Ok((nu, None));
    };
    if moment > nu {
        return Err(format!(
            "op_moment {} uit '{}' ligt na het vastleggen ({}): wat nog moet gebeuren, wordt niet vastgelegd",
            datum::als_op_moment(&moment),
            b.source,
            datum::als_op_moment(&nu)
        ));
    }
    Ok((moment, Some(b.legal_basis.clone())))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    const STROOM: &str = include_str!("../tests/fixtures/chronicles/test_aanvragen.yaml");

    const ZAAKVERLOOP: &str =
        include_str!("../tests/fixtures/chronicles/test_afnemer_zaakverloop.yaml");

    /// Wat een sleutel van het gram zelf bij een event is: vast in de stroom,
    /// afhankelijk van het gram, of nooit; een andere sleutel is een veldpad.
    #[test]
    fn het_kenmerk_van_een_event() {
        let s = parse(ZAAKVERLOOP, "fixture").unwrap();
        let (decision, bekend) = (
            s.events
                .iter()
                .find(|e| e.name == "besluit_genomen")
                .unwrap(),
            s.events
                .iter()
                .find(|e| e.name == "besluit_bekendgemaakt")
                .unwrap(),
        );
        assert_eq!(
            bekend.kenmerk(&s, "refers_to.decision"),
            Some(Eventkenmerk::Vrij)
        );
        assert_eq!(
            bekend.kenmerk(&s, "refers_to.amends"),
            Some(Eventkenmerk::Nooit)
        );
        assert_eq!(bekend.kenmerk(&s, "root"), Some(Eventkenmerk::Vrij));
        assert_eq!(bekend.kenmerk(&s, "zaak"), None);
        assert_eq!(
            decision.kenmerk(&s, "legal_character"),
            Some(Eventkenmerk::Vrij)
        );
        assert_eq!(bekend.kenmerk(&s, "content.naam"), None);
    }

    const ZAAK: &str = "00000000-0000-4000-8000-000000000001";

    fn moment() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2025-03-12T10:14:03+01:00").unwrap()
    }

    fn intake() -> Value {
        json!({"channel": "portaal", "eherkenning": {"kvk": "12345678", "persoon": "A. Tester"}, "burger": {"nummer": null}})
    }

    #[test]
    fn fixture_laadt_en_heeft_een_hash() {
        let s = parse(STROOM, "fixture").unwrap();
        assert_eq!(s.id, "test_aanvragen");
        assert_eq!(s.sha256.len(), 64);
        assert_eq!(s.events[0].legal_basis, vec!["testregeling_aanvraag#1"]);
    }

    #[test]
    fn aanvraag_zonder_vaste_kern_faalt_op_het_schema() {
        let tekst = STROOM.replace("        dagtekening: $external.dagtekening\n", "");
        let error = parse(&tekst, "t").unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("/events/0/fields/core")),
            "{error:?}"
        );
    }

    #[test]
    fn ongeldige_stroom_faalt_op_het_schema() {
        let error = parse(
            "$id: x\nrecording_actor: y\nchronicle: z\nevents: []\n",
            "t",
        )
        .unwrap_err();
        assert!(error[0].contains("/events"), "{error:?}");
    }

    #[test]
    fn bladeren_in_documentvolgorde() {
        let s = parse(STROOM, "fixture").unwrap();
        let paden: Vec<String> = s.events[0].bladeren().into_iter().map(|b| b.path).collect();
        assert_eq!(paden[0], "core.aanvrager.naam");
        assert!(paden.contains(&"content.organen".to_string()));
        assert!(s.events[0].heeft_pad("core.signed_via"));
        assert!(!s.events[0].heeft_blad("core.signed_via"));
        assert!(!s.events[0].heeft_pad("content.bestaat_niet"));
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
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
        .unwrap();
        assert_eq!(gram.type_, "submission");
        assert_eq!(gram.subtype.as_deref(), Some("aanvraag"));
        assert_eq!(gram.effective_at, "2025-03-12T10:14:03+01:00");
        assert_eq!(
            gram.field("core.signed_via.kvk_nummer"),
            Some(&json!("12345678"))
        );
        // Een $external-waarde voedt twee velden.
        assert_eq!(
            gram.field("content.naam"),
            Some(&json!("Vereniging Voorbeeld"))
        );
        assert_eq!(
            gram.field("core.aanvrager.naam"),
            Some(&json!("Vereniging Voorbeeld"))
        );
        // Een constante van de stroom.
        assert_eq!(
            gram.field("core.gevraagde_beschikking"),
            Some(&json!("testbeschikking, testregeling artikel 1"))
        );
        // Niet ingevuld: vastgelegd als null, de vorm blijft.
        assert_eq!(gram.field("content.aanduiding"), Some(&Value::Null));
        gram.valideer().unwrap();
    }

    fn met_intake(s: &Stroom, intake: Value) -> Result<Gram, String> {
        bouw_gram(
            s,
            &s.events[0],
            &Indiening {
                intake: &intake,
                external: &Map::new(),
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
    }

    /// Twee tijden: zonder opgegeven ontvangst is het op_moment het
    /// vastleggen; met een datumstempel van het loket is het die dag, met de
    /// grondslag uit de stroom, en blijft vastgelegd_op de klok.
    #[test]
    fn op_moment_uit_een_opgegeven_ontvangst() {
        let s = parse(STROOM, "fixture").unwrap();
        let g = met_intake(&s, intake()).unwrap();
        assert_eq!(g.effective_at, "2025-03-12T10:14:03+01:00");
        assert_eq!(g.recorded_at, "2025-03-12T10:14:03+01:00");
        assert_eq!(g.effective_at_legal_basis, None);

        let mut counter = intake();
        counter["received_at"] = json!("2025-03-05");
        let g = met_intake(&s, counter.clone()).unwrap();
        assert_eq!(g.effective_at, "2025-03-05T00:00:00+01:00");
        assert_eq!(g.recorded_at, "2025-03-12T10:14:03+01:00");
        assert_eq!(
            g.effective_at_legal_basis,
            Some(vec!["testregeling_aanvraag#1".to_string()])
        );
        g.valideer().unwrap();

        // Een moment met tijdzone mag ook; null is: niet opgegeven.
        counter["received_at"] = json!("2025-03-05T16:45:00+01:00");
        assert_eq!(
            met_intake(&s, counter.clone()).unwrap().effective_at,
            "2025-03-05T16:45:00+01:00"
        );
        counter["received_at"] = Value::Null;
        assert_eq!(
            met_intake(&s, counter.clone())
                .unwrap()
                .effective_at_legal_basis,
            None
        );

        // Na het vastleggen, of geen datum: geweigerd.
        counter["received_at"] = json!("2025-03-13");
        let f = met_intake(&s, counter.clone()).unwrap_err();
        assert!(f.contains("ligt na het vastleggen"), "{f}");
        counter["received_at"] = json!("vorige week");
        let f = met_intake(&s, counter.clone()).unwrap_err();
        assert!(
            f.contains("ongeldig op_moment uit '$intake.received_at'"),
            "{f}"
        );
        counter["received_at"] = json!(20250305);
        let f = met_intake(&s, counter).unwrap_err();
        assert!(f.contains("geen datum of moment"), "{f}");
    }

    /// Wat het ontvangstmoment niet zelf mag kiezen, bindt aan `$intake`: een
    /// `ontvangen_op` onder `external` is een onbekend veld.
    #[test]
    fn de_indiener_kiest_de_ontvangst_niet() {
        let s = parse(STROOM, "fixture").unwrap();
        let f = bouw(&s, json!({"received_at": "2025-03-01"})).unwrap_err();
        assert!(f.contains("onbekend veld 'received_at'"), "{f}");
    }

    #[test]
    fn onbekend_veld_wordt_geweigerd() {
        let s = parse(STROOM, "fixture").unwrap();
        let external = json!({"schoenmaat": 44});
        let error = bouw_gram(
            &s,
            &s.events[0],
            &Indiening {
                intake: &intake(),
                external: external.as_object().unwrap(),
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("'schoenmaat'"), "{error}");
    }

    fn bouw(s: &Stroom, external: Value) -> Result<Gram, String> {
        bouw_gram(
            s,
            &s.events[0],
            &Indiening {
                intake: &intake(),
                external: external.as_object().unwrap(),
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
    }

    #[test]
    fn tabelveld_declareert_zijn_kolommen() {
        let s = parse(STROOM, "fixture").unwrap();
        let e = &s.events[0];
        assert_eq!(
            e.columns("content.organen").unwrap(),
            vec!["orgaan", "zetels", "samengevoegd", "aantal_aanduidingen"]
        );
        assert_eq!(e.columns("content.naam"), None);
        assert!(e.heeft_blad("content.organen"));
    }

    #[test]
    fn onbekende_kolom_wordt_geweigerd_met_veldpad() {
        let s = parse(STROOM, "fixture").unwrap();
        let error = bouw(
            &s,
            json!({"organen": [{"orgaan": "raad"}, {"orgaan": "raad", "kleur": "rood"}]}),
        )
        .unwrap_err();
        assert!(
            error.contains("onbekend veld 'organen[1].kleur'"),
            "{error}"
        );
    }

    #[test]
    fn tabel_krijgt_elke_kolom_in_het_gram() {
        let s = parse(STROOM, "fixture").unwrap();
        let gram = bouw(&s, json!({"organen": [{"zetels": 3, "orgaan": "raad"}]})).unwrap();
        assert_eq!(
            gram.field("content.organen"),
            Some(
                &json!([{"orgaan": "raad", "zetels": 3, "samengevoegd": null, "aantal_aanduidingen": null}])
            )
        );
        gram.valideer().unwrap();
        // Niet ingevuld: null, net als een gewoon veld.
        let gram = bouw(&s, json!({})).unwrap();
        assert_eq!(gram.field("content.organen"), Some(&Value::Null));
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
            let error = bouw(&s, external).unwrap_err();
            assert!(error.contains(verwacht), "{error}");
        }
    }

    #[test]
    fn onbekend_veld_in_een_genest_external_object() {
        let tekst = STROOM.replace("adres: $external.adres", "adres: $external.adres.straat");
        let s = parse(&tekst, "t").unwrap();
        let gram = bouw(&s, json!({"adres": {"straat": "Voorbeeldstraat 1"}})).unwrap();
        assert_eq!(
            gram.field("core.aanvrager.adres"),
            Some(&json!("Voorbeeldstraat 1"))
        );
        let error = bouw(&s, json!({"adres": {"straat": "x", "huisdier": "kat"}})).unwrap_err();
        assert!(error.contains("onbekend veld 'adres.huisdier'"), "{error}");
    }

    #[test]
    fn een_bronpad_met_twee_vormen_faalt_bij_het_laden() {
        let tekst = STROOM.replace(
            "rekeningnummer: $external.rekeningnummer",
            "rekeningnummer: $external.organen",
        );
        let error = parse(&tekst, "t").unwrap_err();
        assert!(
            error
                .iter()
                .any(|f| f.contains("'$external.organen' krijgt meer dan een vorm")),
            "{error:?}"
        );
    }

    #[test]
    fn tabel_zonder_kolommen_faalt_op_het_schema() {
        let tekst = STROOM.replace(
            "          columns: [orgaan, zetels, samengevoegd, aantal_aanduidingen]\n",
            "          columns: []\n",
        );
        let error = parse(&tekst, "t").unwrap_err();
        assert!(
            error
                .iter()
                .any(|f| f.contains("/events/0/fields/content/organen")),
            "{error:?}"
        );
    }

    /// De verwijzingen die het proces meegeeft, passen bij het event: een
    /// onbekende naam en een ontbrekende verplichte verwijzing weigeren. De
    /// rollen (besluit, volgt, wortel) volgen uit stage en verwijzingen.
    #[test]
    fn verwijzingen_en_rollen() {
        let mut streams = vec![
            parse(
                include_str!("../tests/fixtures/chronicles/test_afnemer_aanvragen.yaml"),
                "a",
            )
            .unwrap(),
            parse(ZAAKVERLOOP, "v").unwrap(),
        ];
        assert!(controleer_verwijzingen(&streams).is_empty());
        leid_rollen_af(&mut streams);
        let e = |n: &str| {
            streams
                .iter()
                .flat_map(|s| s.events.iter())
                .find(|e| e.name == n)
                .unwrap()
                .clone()
        };
        assert_eq!(e("aanvraag_ontvangen").case, Zaak::Opens);
        assert_eq!(e("besluit_genomen").decision, Some(Decision::Opens));
        assert_eq!(e("betaling_verricht").decision, Some(Decision::Follows));
        assert_eq!(e("aanvulling_gevraagd").decision, None);
        assert_eq!(e("aanvulling_gevraagd").case, Zaak::Follows);
        let betaling = e("betaling_verricht");
        let leeg = BTreeMap::new();
        assert!(toets_verwijzingen(&betaling, &leeg)
            .unwrap_err()
            .contains("verplicht met 'decision'"));
        let mut v = BTreeMap::new();
        v.insert("application".to_string(), ZAAK.to_string());
        assert!(toets_verwijzingen(&betaling, &v)
            .unwrap_err()
            .contains("verwijst niet met 'application'"));
        v.clear();
        v.insert("decision".to_string(), ZAAK.to_string());
        toets_verwijzingen(&betaling, &v).unwrap();
        // Een verwijzing waar geen event van de cel bij past.
        let los = parse(
            &ZAAKVERLOOP.replace("to: aanvraag_ontvangen", "to: bestaat_niet"),
            "v",
        )
        .unwrap();
        assert!(!controleer_verwijzingen(&[los]).is_empty());
    }

    #[test]
    fn ontbrekende_intake_is_een_fout() {
        let s = parse(STROOM, "fixture").unwrap();
        let external = Map::new();
        let error = bouw_gram(
            &s,
            &s.events[0],
            &Indiening {
                intake: &json!({"channel": "portaal"}),
                external: &external,
                recorded_at: moment(),
                refers_to: &BTreeMap::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("$intake.eherkenning.kvk"), "{error}");
    }
}
