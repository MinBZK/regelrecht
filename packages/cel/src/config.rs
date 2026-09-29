//! Configuratie: de omgeving van de runtime, de celdefinitie (`cel.yaml`) en
//! de procesdefinitie (`proces.yaml`).
//!
//! Een cel is een map onder `CELLS_PATH` met een `cel.yaml`, een proces een
//! map onder `PROCESSES_PATH` met een `proces.yaml`. Paden daarin zijn
//! relatief aan die map.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::kanaal::{KanaalDefinitie, RolDefinitie, Routes};
use crate::laden;
use crate::schema::Soort;

/// Standaardpoort, binnen 7100-7300.
pub const STANDAARD_POORT: u16 = 7170;

/// De naam van het bestand dat van een map een cel maakt.
pub const CEL_BESTAND: &str = "cell.yaml";

/// De naam van het bestand dat van een map een proces maakt.
pub const PROCES_BESTAND: &str = "process.yaml";

/// De omgeving van de runtime.
#[derive(Debug, Clone)]
pub struct Config {
    /// Map met een submap per cel, elk met een `cel.yaml`.
    pub cells_path: PathBuf,
    /// Map met een submap per proces, elk met een `proces.yaml`. Zonder:
    /// geen processen, alleen cellen.
    pub processes_path: Option<PathBuf>,
    /// Map met de regelingen (het corpus), gedeeld door alle cellen.
    pub regulation_path: PathBuf,
    /// Map voor de kronieken: per cel een submap `<id>/`.
    pub data_dir: PathBuf,
    pub port: u16,
    /// Het leestoken (`CEL_LEES_TOKEN`) dat runtimes delen die elkaars
    /// cellen mogen lezen; zonder leest alleen de eigen runtime.
    pub lees_token: Option<String>,
    /// De runtimes (basis-urls) die het leestoken meekrijgen
    /// (`CEL_LEES_TOKEN_BRONNEN`, komma's ertussen). Een bron met een andere
    /// url krijgt het niet: het token geeft lezen in deze runtime.
    pub lees_token_bronnen: Vec<String>,
    /// Langs welke route de cellen reduceren (`CEL_REDUCTIE`, experiment A).
    pub reduction: Reductiemodus,
    /// Het koppelbestand van de registers (`CEL_REGISTERS`): welk systeem
    /// het register levert dat een beleid bevraagt (zie
    /// [`crate::register`]). Zonder: geen registers, en een beleid dat er een
    /// bevraagt houdt de runtime tegen.
    pub registers: Option<PathBuf>,
}

/// Hoe de cellen van de runtime een lexostatus reduceren (`CEL_REDUCTIE`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Reductiemodus {
    /// De reductie-DSL (`dsl`, de standaard).
    #[default]
    Dsl,
    /// Elke lexostatus als engine-run van de regeling die het koppelbestand
    /// (`CEL_ENGINE_KOPPELING`) noemt (`engine`); zie
    /// [`crate::lexostatus_engine`]. Met `vergelijk` reduceert de cel ook
    /// langs de DSL en is elk verschil een fout.
    Engine { koppeling: PathBuf, vergelijk: bool },
}

impl Reductiemodus {
    /// Uit `CEL_REDUCTIE` en `CEL_ENGINE_KOPPELING`. Een koppelbestand zonder
    /// engine-route, of de engine-route zonder koppelbestand, is een fout:
    /// geen stille terugval.
    pub fn uit(reduction: Option<&str>, koppeling: Option<&str>) -> Result<Self, String> {
        let koppeling = koppeling.map(str::trim).filter(|k| !k.is_empty());
        let engine = |vergelijk| match koppeling {
            Some(k) => Ok(Self::Engine {
                koppeling: PathBuf::from(k),
                vergelijk,
            }),
            None => Err(
                "CELL_REDUCTION vraagt de engine, maar CELL_ENGINE_BINDING is niet gezet"
                    .to_string(),
            ),
        };
        match reduction.map(str::trim).unwrap_or("") {
            "" | "dsl" => match koppeling {
                None => Ok(Self::Dsl),
                Some(_) => Err("CELL_ENGINE_BINDING is gezet, maar CELL_REDUCTION is niet 'engine' of 'compare'".into()),
            },
            "engine" => engine(false),
            "compare" => engine(true),
            anders => Err(format!(
                "CELL_REDUCTION '{anders}' is geen 'dsl', 'engine' of 'compare'"
            )),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        fn path(name: &str) -> Result<PathBuf, String> {
            std::env::var(name)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} is niet gezet"))
        }
        let port = match std::env::var("CELL_PORT") {
            Ok(v) => v
                .parse()
                .map_err(|_| format!("CELL_PORT '{v}' is geen poortnummer"))?,
            Err(_) => STANDAARD_POORT,
        };
        Ok(Self {
            cells_path: path("CELLS_PATH")?,
            processes_path: path("PROCESSES_PATH").ok(),
            regulation_path: path("REGULATION_PATH")?,
            data_dir: path("DATA_DIR")?,
            port,
            lees_token: match std::env::var("CELL_READ_TOKEN") {
                Ok(t) if t.trim().len() >= 16 => Some(t.trim().to_string()),
                Ok(t) if !t.trim().is_empty() => {
                    return Err("CELL_READ_TOKEN is korter dan 16 tekens".into())
                }
                _ => None,
            },
            lees_token_bronnen: std::env::var("CELL_READ_TOKEN_SOURCES")
                .unwrap_or_default()
                .split(',')
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty())
                .collect(),
            reduction: Reductiemodus::uit(
                std::env::var("CELL_REDUCTION").ok().as_deref(),
                std::env::var("CELL_ENGINE_BINDING").ok().as_deref(),
            )?,
            registers: path("CELL_REGISTERS").ok(),
        })
    }
}

/// Een celdefinitie (`schema/chronolex/v0.2.0/cel.json`): alleen wat de
/// cel zelf doet. Vastleggen (de stromen), bewaren (de kronieken) en
/// reduceren (de lexostatussen).
#[derive(Debug, Clone, Deserialize)]
pub struct CelDefinitie {
    pub id: String,
    pub recording_actor: String,
    pub streams: Vec<String>,
    pub lexostatuses: String,
    #[serde(default)]
    pub initial_state: Option<String>,
}

/// Een procesdefinitie (`schema/chronolex/v0.2.0/proces.json`): wie er
/// handelt en hoe. Informeren (synthese, toets, aanbod), concluderen (het
/// besluit) en een cel laten vastleggen.
#[derive(Debug, Clone, Deserialize)]
pub struct ProcesDefinitie {
    pub id: String,
    /// De actor van het proces. Een cel legt voor het proces alleen vast in
    /// een stroom met deze `recording_actor`, en het besluit is de
    /// beschikking waarvoor deze actor bevoegd is.
    pub actor: String,
    /// Hoe streng de controle op de herkomst is (zie [`crate::origin`]).
    #[serde(default)]
    pub origin_check: Herkomstcontrole,
    /// Namens welk bevoegd gezag het proces handelt (zie [`crate::gezag`]).
    /// Nodig voor een besluit; zonder telt geen uitvoeringsbeleid als dat van
    /// de actor.
    #[serde(default)]
    pub on_behalf_of: Option<Namens>,
    /// Gezagen waarvoor het proces in mandaat handelt (Awb 10:1), elk met
    /// een grondslag.
    #[serde(default)]
    pub mandates: Vec<Mandaat>,
    /// Langs welke kanalen iemand inlogt (zie [`crate::kanaal`]).
    #[serde(default)]
    pub channels: BTreeMap<String, KanaalDefinitie>,
    /// Wie er inlogt, langs welk kanaal, en welke routes die rol mag. Zonder
    /// rollen is er geen login.
    #[serde(default)]
    pub roles: BTreeMap<String, RolDefinitie>,
    #[serde(default)]
    pub portal: Option<Portal>,
    #[serde(default)]
    pub synthesis: Vec<SyntheseBron>,
    #[serde(default)]
    pub handling: Option<Handling>,
    /// Standaardgegevens per handeling, voor een proefopstelling.
    #[serde(default)]
    pub examples: Option<VoorbeeldenDefinitie>,
}

/// Hoe streng de controle op de herkomst (RFC-043) is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Herkomstcontrole {
    /// Een parameter zonder origin is een waarschuwing.
    #[default]
    Lenient,
    /// Een parameter zonder origin is een fout: wie hem levert, is niet na
    /// te gaan.
    Strict,
}

/// Het blok `voorbeelden`: per handeling een JSON-bestand, relatief aan de
/// map van het proces (zie [`crate::voorbeelden`]).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VoorbeeldenDefinitie {
    /// Logins, elk een object met de velden van een kanaal en optioneel
    /// `kanaal` en `rol`.
    #[serde(default)]
    pub logins: Vec<String>,
    /// Een aanvraag: `{external: {...}}`.
    #[serde(default)]
    pub application: Option<String>,
    /// Per handeling een formulier: `{formulier: {...}}`.
    #[serde(default)]
    pub actions: BTreeMap<String, String>,
}

/// Namens welk bevoegd gezag het proces handelt: een naam zoals een
/// regeling hem in `competent_authority` noemt, of een regeling waarvan het
/// bevoegd gezag het is.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Namens {
    Gezag { authority: String },
    Regeling { regulation: String },
}

/// Een mandaat (Awb 10:1): het proces handelt ook namens dit gezag, op grond
/// van `grondslag` (`<regeling>#<artikel>`).
#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct Mandaat {
    pub authority: String,
    pub legal_basis: String,
}

/// Wat de behandelaar in het proces doet: een werkvoorraad, en handelingen
/// in een zaak.
#[derive(Debug, Clone, Deserialize)]
pub struct Handling {
    /// Een lijst-lexostatus van de cel van het proces.
    pub worklist: LexostatusVerwijzing,
    pub actions: Vec<HandelingDefinitie>,
}

impl Handling {
    /// De handeling met deze naam.
    pub fn action(&self, name: &str) -> Option<&HandelingDefinitie> {
        self.actions.iter().find(|h| h.name == name)
    }
}

/// Een lexostatus van een cel.
#[derive(Debug, Clone, Deserialize)]
pub struct LexostatusVerwijzing {
    pub cell: String,
    pub lexostatus: String,
}

/// Een handeling in een zaak (zie [`crate::handeling`]): de uitkomsten van
/// een artikel, en het event waarin de cel haar vastlegt. Wat de handeling
/// nodig heeft en van wie, staat niet in `proces.yaml`: het volgt bij het
/// laden uit de stage van het event (RFC-008) en uit de origin van de
/// parameters (RFC-043); zie de velden zonder serde hieronder.
#[derive(Debug, Clone, Deserialize)]
pub struct HandelingDefinitie {
    /// Uniek in het proces; de route is `zaken/<z>/handelingen/<naam>`.
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    /// De rol die de handeling mag doen (een sleutel van `rollen`, met
    /// routes `behandeling`). Zonder: elke rol die de behandeling mag.
    #[serde(default)]
    pub role: Option<String>,
    /// De handeling van het besluit waarbij deze handeling hoort: bij een
    /// feit dat een besluit volgt (een betaling die het uitvoert) en bij een
    /// besluit dat een ander wijzigt. Het proces handelt dan op het laatste
    /// besluit van die handeling in de zaak, een wijziging ervan
    /// meegerekend. Een vervolg vindt zijn besluit zelf (zie
    /// [`Handelingsoort::Vervolg`]).
    #[serde(default)]
    pub decision: Option<String>,
    /// De parameter van het artikel die het id krijgt van het besluit
    /// waarop de handeling handelt (notitie bron en gram-id): zo roept het
    /// proces het eigen beleid aan dat per besluit leest, zoals de
    /// betalingsadministratie. Bedrading, geen wet: welke parameter het is,
    /// zegt het proces.
    #[serde(default)]
    pub decision_parameter: Option<String>,
    /// Leeg in `proces.yaml`: de runtime vult haar bij het laden met de
    /// regeling van de beschikking waarvoor het gezag van het proces
    /// (`namens`) bevoegd is (zie [`crate::gezag::beschikkingen_van`]).
    #[serde(default)]
    pub regulation: String,
    /// Uitkomsten van een artikel. Bij een vervolg komen de uitkomsten van
    /// de haken van die stage er bij het laden bij.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Synthese per regel: een tabelveld wordt een array-parameter.
    #[serde(default)]
    pub rows: Vec<RijenDefinitie>,
    /// Waar de handeling als gram wordt vastgelegd.
    pub record: Vastleggen,
    /// Het artikel van de uitkomsten, als `<regeling>#<artikel>`; bij het
    /// laden gezet.
    #[serde(skip)]
    pub article: String,
    /// Wat voor handeling het is; afgeleid uit het event en de procedure.
    #[serde(skip)]
    pub soort: Handelingsoort,
    /// De stage van het vastleg-event, als het er een heeft.
    #[serde(skip)]
    pub stage: Option<String>,
    /// Of het vastleg-event een besluit opent, volgt of wijzigt.
    #[serde(skip)]
    pub decision_role: Option<crate::stroom::Decision>,
    /// De oordelen: de parameters van het artikel met origin `OORDEEL`
    /// (zie [`crate::origin::oordelen`]). Niet bij een vervolg: die oordelen
    /// gaf de behandelaar bij het besluit.
    #[serde(skip)]
    pub verdicts: Vec<Oordeel>,
    /// De feiten die de handeling vastlegt en die de behandelaar invult: bij
    /// een feit de `$external`-velden van het event die geen uitkomst zijn,
    /// bij een vervolg wat de stage vraagt (`requires`).
    #[serde(skip)]
    pub feiten: Vec<crate::formulier::Veld>,
    /// Feiten die pas in een latere stage ontstaan, met hun stand bij deze
    /// handeling: afgeleid uit de procedure (RFC-008), alleen bij een besluit
    /// en alleen voor wat geen lexostatus van de zaak levert (zie
    /// [`crate::handeling::nog_niet`]).
    #[serde(skip)]
    pub not_yet: BTreeMap<String, NogNiet>,
    /// De booleaanse uitkomsten van een TOETS-artikel dat in de grondslag
    /// van het event staat: onwaar is niet te nemen (zie
    /// [`crate::handeling::toetsen`]).
    #[serde(skip)]
    pub assessments: Vec<String>,
    /// Bij een vervolg: de haken die de wet op die stage laat vuren, als
    /// `<regeling>#<artikel>` (RFC-008).
    #[serde(skip)]
    pub hooks: Vec<String>,
    /// Het type en de eenheid van elke uitkomst en toets, uit de regeling
    /// (zie [`crate::regelingen::Waardetype`]).
    #[serde(skip)]
    pub types: BTreeMap<String, crate::regelingen::Waardetype>,
}

impl HandelingDefinitie {
    /// Hoe de frontend haar noemt.
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

/// Wat voor handeling het is. Het volgt uit het vastleg-event: met een stage
/// is het een besluit of een vervolg op een besluit, zonder een feit.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Handelingsoort {
    /// Een feit uit het verloop van de zaak (een verzoek, een ontvangst, een
    /// betaling): het event heeft geen stage. De lexostatussen van de zaak
    /// lezen het; op proef telt het concept mee.
    #[default]
    Fact,
    /// Het besluit: de eerste stage van de procedure van het artikel die een
    /// handeling vastlegt. Een zaak kan meer besluiten hebben, elk van een
    /// eigen artikel; een besluit dat een ander wijzigt, legt vast in een
    /// event met `besluit: wijzigt`.
    Decision,
    /// Een latere stage van hetzelfde besluit, zoals de bekendmaking: de
    /// engine voert die stage uit op de invoer van het vastgelegde besluit
    /// (RFC-008, `execute_stage`). Dat is het laatste besluit in de zaak dat
    /// de handeling van het besluit vastlegde.
    FollowUp {
        /// De handeling van het besluit.
        decision: String,
        /// De procedure (RFC-008) waarvan beide stages zijn.
        procedure: String,
    },
}

/// Een feit dat bij het besluit nog niet gebeurd is: de stage van de
/// procedure waarin het pas ontstaat, en de stand bij het besluit (onwaar,
/// of leeg).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct NogNiet {
    pub value: Value,
    pub stage: String,
}

/// De cel en het event waarin het proces een handeling laat vastleggen. Het
/// event heeft `zaak: volgt`; zijn `$external`-sleutels zijn uitkomsten van
/// de handeling of velden van haar formulier.
#[derive(Debug, Clone, Deserialize)]
pub struct Vastleggen {
    pub cell: String,
    pub stream: String,
    pub event: String,
}

/// Synthese per regel: voor elke regel van een tabelveld uit een eigen
/// lexostatus bevraagt de cel bronnen met waarden uit die regel, en voegt de
/// kolommen samen tot een array-parameter.
#[derive(Debug, Clone, Deserialize)]
pub struct RijenDefinitie {
    /// De array-parameter die de regels samen vormen.
    pub parameter: String,
    /// Het tabelveld van een lexostatus van de zaak of van een bron die het
    /// doorgeeft (een extra veld of parameter).
    pub table: InvoerVerwijzing,
    /// Per kolom van de tabel: onder welke naam ze in de parameter komt.
    /// Een kolom die hier niet staat, gaat niet mee.
    pub columns: BTreeMap<String, String>,
    /// Bronnen die per regel worden bevraagd.
    #[serde(default)]
    pub sources: Vec<RijBron>,
}

/// Een bron die per regel wordt bevraagd.
#[derive(Debug, Clone, Deserialize)]
pub struct RijBron {
    pub cell: String,
    /// Zonder url: de bron-cel draait in dezelfde runtime (intern transport).
    #[serde(default)]
    pub url: Option<String>,
    pub lexostatus: String,
    /// Per input van de bron: waar de waarde vandaan komt.
    pub input: BTreeMap<String, RijInvoer>,
    /// Per naam die de bron levert: onder welke kolomnaam ze in de regel komt.
    pub columns: BTreeMap<String, String>,
    /// Waarop de vertaling rust: de artikelen die de kolom bij de afnemer
    /// vragen en die de bron haar feit laten leveren, en een vaste waarde in
    /// de invoer (zie [`vertaalt`](RijBron::vertaalt)).
    #[serde(default)]
    pub legal_basis: Vec<String>,
}

impl RijBron {
    /// Wat deze bron vertaalt: een kolom die bij de afnemer anders heet dan
    /// bij de bron, en een vaste waarde in de invoer. Leeg: niets.
    pub fn vertaalt(&self) -> Vec<String> {
        let mut uit: Vec<String> = self
            .columns
            .iter()
            .filter(|(b, a)| b != a)
            .map(|(b, a)| format!("{b} -> {a}"))
            .collect();
        uit.extend(self.input.iter().filter_map(|(n, i)| match i {
            RijInvoer::Waarde { value } => Some(format!("{n} = {value}")),
            _ => None,
        }));
        uit
    }
}

/// Waar de invoer van een bron per regel vandaan komt.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RijInvoer {
    /// Een kolom van de regel zelf, zoals die na de kolomnamen heet.
    Kolom { column: String },
    /// Een veld van een lexostatus van de zaak, of van een bron die het
    /// doorgeeft (een parameter of een extra veld).
    Own { lexostatus: String, field: String },
    /// Een parameter uit de samenvoeging: de lexostatussen van de zaak en de
    /// synthese van het proces.
    Parameter { parameter: String },
    /// Een uitkomst van een regeling, uitgerekend met de samengevoegde
    /// parameters: de wet leidt de invoer af, zoals een peildatum uit een
    /// jaartal. Een keer per uitvoering, voor alle regels.
    Wet { regulation: String, output: String },
    /// Een vaste waarde.
    Waarde { value: Value },
}

/// Waar de invoer van een synthese-bron vandaan komt.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum BronInvoer {
    /// Een veld van een lexostatus van de zaak, of van een eerdere bron.
    Veld(InvoerVerwijzing),
    /// Een vaste waarde, zoals het orgaan waarvan het register wordt gevraagd.
    Waarde { value: Value },
}

impl BronInvoer {
    /// Het veld, als de invoer er een aanwijst.
    pub fn field(&self) -> Option<&InvoerVerwijzing> {
        match self {
            BronInvoer::Veld(v) => Some(v),
            BronInvoer::Waarde { .. } => None,
        }
    }
}

/// De parameters die een synthese-bron levert: per naam bij de bron de naam
/// bij de afnemer. De bron spreekt de taal van haar eigen wet; de vertaling
/// hoort bij de afnemer. In `proces.yaml` een lijst (dezelfde naam) of een
/// tabel (bron: afnemer).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parameters(Vec<(String, String)>);

impl Parameters {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Paren (naam bij de bron, naam bij de afnemer).
    pub fn paren(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(b, a)| (b.as_str(), a.as_str()))
    }

    /// De namen bij de afnemer.
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(_, a)| a)
    }

    /// De paren waarin de afnemer het feit onder een andere naam vraagt.
    pub fn vertaald(&self) -> BTreeMap<&str, &str> {
        self.paren().filter(|(b, a)| b != a).collect()
    }
}

/// Als lijst van de namen bij de afnemer: wat de bron het proces levert.
impl serde::Serialize for Parameters {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}

/// Over de namen bij de afnemer: de parameters die de bron het proces levert.
impl<'a> IntoIterator for &'a Parameters {
    type Item = &'a String;
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (String, String)>,
        fn(&'a (String, String)) -> &'a String,
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(_, a)| a)
    }
}

impl<'de> Deserialize<'de> for Parameters {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Vorm {
            Lijst(Vec<String>),
            Tabel(BTreeMap<String, String>),
        }
        Ok(Parameters(match Vorm::deserialize(d)? {
            Vorm::Lijst(l) => l.into_iter().map(|n| (n.clone(), n)).collect(),
            Vorm::Tabel(t) => t.into_iter().collect(),
        }))
    }
}

/// Een veld van het besluitformulier: een parameter met een label, uit de
/// regeling.
#[derive(Debug, Clone, PartialEq)]
pub struct Oordeel {
    pub parameter: String,
    pub label: String,
    pub group: Option<String>,
    pub explanation: Option<String>,
}

/// Het portaalblok: in welke cel en welk event een indiening wordt, en welke
/// uitkomst de toets vraagt.
#[derive(Debug, Clone, Deserialize)]
pub struct Portal {
    pub cell: String,
    pub stream: String,
    pub event: String,
    pub assessment: Toets,
    /// Wat het portaal aanbiedt: een uitkomst van het beleid van de actor,
    /// met optioneel de termijn die erbij getoond wordt.
    #[serde(default)]
    pub offer: Option<Aanbod>,
    #[serde(default)]
    pub form: Option<FormulierVerwijzing>,
}

/// Het aanbod van een portaal: een uitkomst van een regeling, uitgevoerd in
/// een run, en optioneel een tweede uitkomst van dezelfde regeling die de
/// termijn geeft.
#[derive(Debug, Clone, Deserialize)]
pub struct Aanbod {
    pub regulation: String,
    pub output: String,
    #[serde(default)]
    pub deadline: Option<String>,
    /// Een uitkomst van dezelfde regeling: de tijdvakken die het beleid
    /// aanbiedt, als het aanbod-artikel een tijdvak vraagt (de parameter met
    /// origin BELANGHEBBENDE en `rol: TIJDVAK`). Het portaal rekent
    /// haar uit in een run zonder parameters op de datum van vandaag.
    #[serde(default)]
    pub windows: Option<String>,
    /// Een uitkomst van dezelfde regeling: de eerste dag van een tijdvak, met
    /// het tijdvak als enige parameter. Het aanbod voor een tijdvak dat nog
    /// moet beginnen, peilt de registers op die dag; zonder op vandaag.
    #[serde(default)]
    pub start: Option<String>,
    /// Een uitkomst van dezelfde regeling: de eerste dag waarop een aanvraag
    /// voor een tijdvak kan binnenkomen, met het tijdvak als enige parameter.
    /// Een loket voert geen ontvangst in van vóór die dag.
    #[serde(default)]
    pub opening: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Toets {
    pub lexostatus: String,
    pub regulation: String,
    pub output: String,
    /// Synthese per regel, zoals bij het besluit: een tabelveld van de
    /// toets-lexostatus wordt een array-parameter.
    #[serde(default)]
    pub rows: Vec<RijenDefinitie>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormulierVerwijzing {
    pub path: String,
    pub screen: String,
}

/// Een lexostatus van een cel die het proces samenvoegt (synthese).
///
/// Een bron met `zaak: true` is een lexostatus van de zaak zelf, in de cel
/// waarin het proces vastlegt: het proces bevraagt haar met het wortel,
/// en ze levert al haar parameters en extra velden. Elke andere bron noemt
/// haar invoer en haar parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct SyntheseBron {
    /// De cel die de lexostatus levert. Leeg bij een bron met `regeling`.
    #[serde(default)]
    pub cell: String,
    /// In plaats van een cel: het eigen beleid van de afnemer, door de
    /// engine uitgerekend (notitie bron en gram-id; zie
    /// [`crate::synthese::Beleidsbron`]). `lexostatus` is dan het artikel,
    /// de invoer zijn de parameters en `extra_velden` de uitkomsten.
    #[serde(default)]
    pub regulation: Option<String>,
    /// Zonder url: de bron-cel draait in dezelfde runtime (intern transport).
    #[serde(default)]
    pub url: Option<String>,
    pub lexostatus: String,
    /// Een lexostatus van de zaak, met als enige input `wortel`.
    #[serde(default)]
    pub case: bool,
    /// Per input van de bron: uit welk veld van een lexostatus van de zaak
    /// (bij de toets: de toets-lexostatus), van een eerdere bron, of een vaste
    /// waarde.
    #[serde(default)]
    pub input: BTreeMap<String, BronInvoer>,
    /// De parameters die deze bron levert, expliciet, met de naam bij de
    /// afnemer.
    #[serde(default)]
    pub parameters: Parameters,
    /// Velden uit het antwoord die geen parameter zijn, maar invoer voor een
    /// latere bron (bijvoorbeeld een naam bij een registratienummer).
    #[serde(default)]
    pub extra_fields: Vec<String>,
    /// Waarop de vertaling rust: de artikelen die het feit bij de afnemer
    /// onder zijn naam vragen en die de bron het laten leveren, en die een
    /// vaste waarde in de invoer dragen (zie [`vertaalt`](SyntheseBron::vertaalt)).
    #[serde(default)]
    pub legal_basis: Vec<String>,
}

impl SyntheseBron {
    /// Wat deze bron vertaalt: een parameter die bij de afnemer anders heet
    /// dan bij de bron, en een vaste waarde in de invoer. Leeg: niets.
    pub fn vertaalt(&self) -> Vec<String> {
        let mut uit: Vec<String> = self
            .parameters
            .vertaald()
            .into_iter()
            .map(|(b, a)| format!("{b} -> {a}"))
            .collect();
        uit.extend(self.input.iter().filter_map(|(n, i)| match i {
            BronInvoer::Waarde { value } => Some(format!("{n} = {value}")),
            BronInvoer::Veld(_) => None,
        }));
        uit
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvoerVerwijzing {
    pub lexostatus: String,
    pub field: String,
}

impl CelDefinitie {
    /// Lees een celdefinitie uit tekst en valideer haar tegen het schema.
    pub fn parse(tekst: &str, source: &str) -> Result<Self, Vec<String>> {
        laden::definitie(tekst, source, Soort::Cell)
    }

    /// Laad `cel.yaml` uit de map van een cel.
    pub fn laad(map: &Path) -> Result<Self, Vec<String>> {
        laden::laad(&map.join(CEL_BESTAND), Self::parse)
    }
}

impl ProcesDefinitie {
    /// Lees een procesdefinitie uit tekst en valideer haar tegen het schema.
    pub fn parse(tekst: &str, source: &str) -> Result<Self, Vec<String>> {
        laden::definitie(tekst, source, Soort::Proces)
    }

    /// Laad `proces.yaml` uit de map van een proces.
    pub fn laad(map: &Path) -> Result<Self, Vec<String>> {
        laden::laad(&map.join(PROCES_BESTAND), Self::parse)
    }

    /// De rollen die een routegroep mogen gebruiken.
    pub fn rollen_met(&self, r: Routes) -> impl Iterator<Item = (&String, &RolDefinitie)> {
        self.roles.iter().filter(move |(_, d)| d.mag(r))
    }

    /// De kanalen van de rollen die een routegroep mogen gebruiken, elk een
    /// keer, met hun id.
    pub fn kanalen_met(&self, r: Routes) -> Vec<(&str, &KanaalDefinitie)> {
        let mut uit: Vec<(&str, &KanaalDefinitie)> = Vec::new();
        for (_, role) in self.rollen_met(r) {
            if let Some((id, k)) = self.channels.get_key_value(&role.channel) {
                if !uit.iter().any(|(i, _)| *i == id) {
                    uit.push((id, k));
                }
            }
        }
        uit
    }

    /// De bronnen van de zaak (`zaak: true`), in de volgorde van de synthese.
    pub fn zaakbronnen(&self) -> impl Iterator<Item = &SyntheseBron> {
        self.synthesis.iter().filter(|b| b.case)
    }

    /// De bronnen die geen lexostatus van de zaak zijn.
    pub fn andere_bronnen(&self) -> impl Iterator<Item = &SyntheseBron> {
        self.synthesis.iter().filter(|b| !b.case)
    }
}

/// De mappen onder `PROCESSES_PATH` met een `proces.yaml`, gesorteerd. Een
/// lege map mag: een runtime met alleen registercellen heeft geen proces.
pub fn procesmappen(processes_path: &Path) -> Result<Vec<PathBuf>, String> {
    laden::mappen_met(processes_path, PROCES_BESTAND)
}

/// De mappen onder `CELLS_PATH` met een `cel.yaml`, gesorteerd.
pub fn celmappen(cells_path: &Path) -> Result<Vec<PathBuf>, String> {
    let mappen = laden::mappen_met(cells_path, CEL_BESTAND)?;
    if mappen.is_empty() {
        return Err(format!(
            "{}: geen submap met een {CEL_BESTAND}",
            cells_path.display()
        ));
    }
    Ok(mappen)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    pub(crate) fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn fixture_cellen_laden() {
        let mappen = celmappen(&fixtures().join("cells")).unwrap();
        let ids: Vec<String> = mappen
            .iter()
            .map(|m| CelDefinitie::laad(m).unwrap().id)
            .collect();
        assert_eq!(
            ids,
            [
                "test_afnemer",
                "test_gebieden",
                "test_instantie",
                "test_register",
                "test_toeslag"
            ]
        );
    }

    #[test]
    fn fixture_processen_laden() {
        let mappen = procesmappen(&fixtures().join("processes")).unwrap();
        let ids: Vec<String> = mappen
            .iter()
            .map(|m| ProcesDefinitie::laad(m).unwrap().id)
            .collect();
        assert_eq!(
            ids,
            [
                "test_afnemer_proces",
                "test_instantie_proces",
                "test_toeslag_proces"
            ]
        );
    }

    #[test]
    fn celdefinitie_valideert_tegen_het_schema() {
        let error =
            CelDefinitie::parse("id: x\nrecording_actor: x\nstreams: []\n", "t").unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("lexostatuses")),
            "{error:?}"
        );
        assert!(error.iter().any(|f| f.contains("/streams")), "{error:?}");
    }

    #[test]
    fn een_cel_heeft_geen_procesblokken() {
        let error = CelDefinitie::parse(
            "id: a\nrecording_actor: a\nstreams: [s.yaml]\nlexostatuses: l.yaml\nroles: {aanvrager: {channel: k, routes: [portal]}}\n",
            "t",
        )
        .unwrap_err();
        assert!(error.iter().any(|f| f.contains("roles")), "{error:?}");
    }

    #[test]
    fn synthese_bron_met_url() {
        let d = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, url: 'http://localhost:7172', lexostatus: l, input: {}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        assert_eq!(d.synthesis[0].url.as_deref(), Some("http://localhost:7172"));
        assert!(d.portal.is_none());
        assert!(d.examples.is_none());
    }

    /// De parameters van een bron: een lijst (dezelfde naam) of per naam bij
    /// de bron de naam bij de afnemer; een invoer is een veld of een vaste
    /// waarde.
    #[test]
    fn parameters_met_vertaling_en_een_vaste_invoer() {
        let d = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, lexostatus: l, input: {x: {lexostatus: e, field: x}, orgaan: {value: raad}}, parameters: {is_ingeschreven_in_register: is_ingeschreven_raad}}\n  - {cell: c, lexostatus: m, input: {}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        let b = &d.synthesis[0];
        assert_eq!(
            b.parameters.paren().collect::<Vec<_>>(),
            [("is_ingeschreven_in_register", "is_ingeschreven_raad")]
        );
        assert_eq!(
            b.parameters.iter().collect::<Vec<_>>(),
            ["is_ingeschreven_raad"]
        );
        assert!(matches!(&b.input["orgaan"], BronInvoer::Waarde { value } if value == "raad"));
        assert_eq!(b.input["x"].field().unwrap().lexostatus, "e");
        assert_eq!(
            d.synthesis[1].parameters.paren().collect::<Vec<_>>(),
            [("p", "p")]
        );
        assert!(d.synthesis[1].parameters.vertaald().is_empty());
    }

    /// De tijdvakken en de stand bij besluit staan niet in de configuratie.
    #[test]
    fn keuzes_en_stand_bij_besluit_worden_geweigerd() {
        let error = ProcesDefinitie::parse(
            "id: a\nactor: a\nportal:\n  cell: a\n  stream: s\n  event: e\n  assessment: {lexostatus: l, regulation: r, output: u}\n  offer: {regulation: r, output: u, keuzes: {jaren_vanaf_nu: [0]}}\n",
            "t",
        )
        .unwrap_err();
        assert!(error.iter().any(|f| f.contains("keuzes")), "{error:?}");
        let error = ProcesDefinitie::parse(
            "id: a\nactor: a\nhandling:\n  worklist: {cell: a, lexostatus: w}\n  actions:\n    - name: b\n      outputs: [u]\n      record: {cell: a, stream: s, event: e}\n      stand_bij_besluit: {x: false}\n",
            "t",
        )
        .unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("stand_bij_besluit")),
            "{error:?}"
        );
    }

    #[test]
    fn een_bron_van_de_zaak_noemt_geen_invoer_of_parameters() {
        let d = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, lexostatus: l, case: true}\n  - {cell: c, lexostatus: m, input: {x: {lexostatus: l, field: x}}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        assert_eq!(d.zaakbronnen().count(), 1);
        assert_eq!(d.andere_bronnen().count(), 1);
        let error = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, lexostatus: l, case: true, parameters: [p]}\n",
            "t",
        )
        .unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("/synthesis/0")),
            "{error:?}"
        );
        let error = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthesis:\n  - {cell: b, lexostatus: l}\n",
            "t",
        )
        .unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("/synthesis/0")),
            "{error:?}"
        );
    }

    /// Het besluitformulier staat niet in `proces.yaml`: het volgt uit de
    /// parameters met origin OORDEEL.
    #[test]
    fn een_besluitformulier_in_de_configuratie_wordt_geweigerd() {
        let error = ProcesDefinitie::parse(
            "id: a\nactor: a\nhandling:\n  worklist: {cell: a, lexostatus: w}\n  actions:\n    - name: b\n      outputs: [u]\n      record: {cell: a, stream: s, event: e}\n      form: [{parameter: p, label: P}]\n",
            "t",
        )
        .unwrap_err();
        assert!(error.iter().any(|f| f.contains("form")), "{error:?}");
    }

    #[test]
    fn voorbeelden_blok() {
        let d = ProcesDefinitie::parse(
            "id: a\nactor: a\nexamples:\n  logins: [login.json]\n  actions: {besluit: besluit.json}\n",
            "t",
        )
        .unwrap();
        let v = d.examples.unwrap();
        assert_eq!(v.logins, ["login.json"]);
        assert_eq!(v.application, None);
        assert_eq!(v.actions["besluit"], "besluit.json");
        let error =
            ProcesDefinitie::parse("id: a\nactor: a\nexamples:\n  inlog: [login.json]\n", "t")
                .unwrap_err();
        assert!(error.iter().any(|f| f.contains("inlog")), "{error:?}");
    }

    #[test]
    fn map_zonder_cellen() {
        let dir = tempfile::tempdir().unwrap();
        assert!(celmappen(dir.path()).unwrap_err().contains("geen submap"));
        // Zonder processen: geen fout.
        assert!(procesmappen(dir.path()).unwrap().is_empty());
    }
}
