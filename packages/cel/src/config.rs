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
pub const CEL_BESTAND: &str = "cel.yaml";

/// De naam van het bestand dat van een map een proces maakt.
pub const PROCES_BESTAND: &str = "proces.yaml";

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
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        fn pad(naam: &str) -> Result<PathBuf, String> {
            std::env::var(naam)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| format!("{naam} is niet gezet"))
        }
        let port = match std::env::var("CEL_PORT") {
            Ok(v) => v
                .parse()
                .map_err(|_| format!("CEL_PORT '{v}' is geen poortnummer"))?,
            Err(_) => STANDAARD_POORT,
        };
        Ok(Self {
            cells_path: pad("CELLS_PATH")?,
            processes_path: pad("PROCESSES_PATH").ok(),
            regulation_path: pad("REGULATION_PATH")?,
            data_dir: pad("DATA_DIR")?,
            port,
            lees_token: match std::env::var("CEL_LEES_TOKEN") {
                Ok(t) if t.trim().len() >= 16 => Some(t.trim().to_string()),
                Ok(t) if !t.trim().is_empty() => {
                    return Err("CEL_LEES_TOKEN is korter dan 16 tekens".into())
                }
                _ => None,
            },
            lees_token_bronnen: std::env::var("CEL_LEES_TOKEN_BRONNEN")
                .unwrap_or_default()
                .split(',')
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty())
                .collect(),
        })
    }
}

/// Een celdefinitie (`schema/chronolex/v0.1.0/cel.json`): alleen wat de
/// cel zelf doet. Vastleggen (de stromen), bewaren (de kronieken) en
/// reduceren (de lexostatussen).
#[derive(Debug, Clone, Deserialize)]
pub struct CelDefinitie {
    pub id: String,
    pub recording_actor: String,
    pub stromen: Vec<String>,
    pub lexostatussen: String,
    #[serde(default)]
    pub startstand: Option<String>,
}

/// Een procesdefinitie (`schema/chronolex/v0.1.0/proces.json`): wie er
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
    pub herkomst: Herkomstcontrole,
    /// Namens welk bevoegd gezag het proces handelt (zie [`crate::gezag`]).
    /// Nodig voor een besluit; zonder telt geen uitvoeringsbeleid als dat van
    /// de actor.
    #[serde(default)]
    pub namens: Option<Namens>,
    /// Gezagen waarvoor het proces in mandaat handelt (Awb 10:1), elk met
    /// een grondslag.
    #[serde(default)]
    pub mandaten: Vec<Mandaat>,
    /// Langs welke kanalen iemand inlogt (zie [`crate::kanaal`]).
    #[serde(default)]
    pub kanalen: BTreeMap<String, KanaalDefinitie>,
    /// Wie er inlogt, langs welk kanaal, en welke routes die rol mag. Zonder
    /// rollen is er geen login.
    #[serde(default)]
    pub rollen: BTreeMap<String, RolDefinitie>,
    #[serde(default)]
    pub portaal: Option<Portaal>,
    #[serde(default)]
    pub synthese: Vec<SyntheseBron>,
    #[serde(default)]
    pub behandeling: Option<Behandeling>,
    /// Standaardgegevens per handeling, voor een proefopstelling.
    #[serde(default)]
    pub voorbeelden: Option<VoorbeeldenDefinitie>,
}

/// Hoe streng de controle op de herkomst (RFC-043) is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Herkomstcontrole {
    /// Een parameter zonder origin is een waarschuwing.
    #[default]
    Ruim,
    /// Een parameter zonder origin is een fout: wie hem levert, is niet na
    /// te gaan.
    Streng,
}

/// Het blok `voorbeelden`: per handeling een JSON-bestand, relatief aan de
/// map van het proces (zie [`crate::voorbeelden`]).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VoorbeeldenDefinitie {
    /// Logins, elk een object met de velden van een kanaal en optioneel
    /// `kanaal` en `rol`.
    #[serde(default)]
    pub inloggen: Vec<String>,
    /// Een aanvraag: `{external: {...}}`.
    #[serde(default)]
    pub aanvraag: Option<String>,
    /// Per handeling een formulier: `{formulier: {...}}`.
    #[serde(default)]
    pub handelingen: BTreeMap<String, String>,
}

/// Namens welk bevoegd gezag het proces handelt: een naam zoals een
/// regeling hem in `competent_authority` noemt, of een regeling waarvan het
/// bevoegd gezag het is.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Namens {
    Gezag { gezag: String },
    Regeling { regeling: String },
}

/// Een mandaat (Awb 10:1): het proces handelt ook namens dit gezag, op grond
/// van `grondslag` (`<regeling>#<artikel>`).
#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct Mandaat {
    pub gezag: String,
    pub grondslag: String,
}

/// Wat de behandelaar in het proces doet: een werkvoorraad, en handelingen
/// in een zaak.
#[derive(Debug, Clone, Deserialize)]
pub struct Behandeling {
    /// Een lijst-lexostatus van de cel van het proces.
    pub werkvoorraad: LexostatusVerwijzing,
    pub handelingen: Vec<HandelingDefinitie>,
}

impl Behandeling {
    /// De handeling met deze naam.
    pub fn handeling(&self, naam: &str) -> Option<&HandelingDefinitie> {
        self.handelingen.iter().find(|h| h.naam == naam)
    }
}

/// Een lexostatus van een cel.
#[derive(Debug, Clone, Deserialize)]
pub struct LexostatusVerwijzing {
    pub cel: String,
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
    pub naam: String,
    #[serde(default)]
    pub label: Option<String>,
    /// De rol die de handeling mag doen (een sleutel van `rollen`, met
    /// routes `behandeling`). Zonder: elke rol die de behandeling mag.
    #[serde(default)]
    pub rol: Option<String>,
    /// Leeg in `proces.yaml`: de runtime vult haar bij het laden met de
    /// regeling van de beschikking waarvoor het gezag van het proces
    /// (`namens`) bevoegd is (zie [`crate::gezag::beschikkingen_van`]).
    #[serde(default)]
    pub regeling: String,
    /// Uitkomsten van een artikel. Bij een vervolg komen de uitkomsten van
    /// de haken van die stage er bij het laden bij.
    #[serde(default)]
    pub uitkomsten: Vec<String>,
    /// Synthese per regel: een tabelveld wordt een array-parameter.
    #[serde(default)]
    pub rijen: Vec<RijenDefinitie>,
    /// Waar de handeling als gram wordt vastgelegd.
    pub vastleggen: Vastleggen,
    /// Het artikel van de uitkomsten, als `<regeling>#<artikel>`; bij het
    /// laden gezet.
    #[serde(skip)]
    pub artikel: String,
    /// Wat voor handeling het is; afgeleid uit het event en de procedure.
    #[serde(skip)]
    pub soort: Handelingsoort,
    /// De stage van het vastleg-event, als het er een heeft.
    #[serde(skip)]
    pub stage: Option<String>,
    /// De oordelen: de parameters van het artikel met origin `OORDEEL`
    /// (zie [`crate::origin::oordelen`]). Niet bij een vervolg: die oordelen
    /// gaf de behandelaar bij het besluit.
    #[serde(skip)]
    pub oordelen: Vec<Oordeel>,
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
    pub nog_niet: BTreeMap<String, NogNiet>,
    /// De booleaanse uitkomsten van een TOETS-artikel dat in de grondslag
    /// van het event staat: onwaar is niet te nemen (zie
    /// [`crate::handeling::toetsen`]).
    #[serde(skip)]
    pub toetsen: Vec<String>,
    /// Bij een vervolg: de haken die de wet op die stage laat vuren, als
    /// `<regeling>#<artikel>` (RFC-008).
    #[serde(skip)]
    pub haken: Vec<String>,
}

impl HandelingDefinitie {
    /// Hoe de frontend haar noemt.
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.naam)
    }
}

/// Wat voor handeling het is. Het volgt uit het vastleg-event: met een stage
/// is het een besluit of een vervolg op een besluit, zonder een feit.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "soort", rename_all = "lowercase")]
pub enum Handelingsoort {
    /// Een feit uit het verloop van de zaak (een verzoek, een ontvangst, een
    /// betaling): het event heeft geen stage. De lexostatussen van de zaak
    /// lezen het; op proef telt het concept mee.
    #[default]
    Feit,
    /// Het besluit: de eerste stage van de procedure van het artikel die een
    /// handeling vastlegt.
    Besluit,
    /// Een latere stage van hetzelfde besluit, zoals de bekendmaking: de
    /// engine voert die stage uit op de invoer van het vastgelegde besluit
    /// (RFC-008, `execute_stage`).
    Vervolg {
        /// De handeling van het besluit.
        besluit: String,
        /// De procedure (RFC-008) waarvan beide stages zijn.
        procedure: String,
    },
}

/// Een feit dat bij het besluit nog niet gebeurd is: de stage van de
/// procedure waarin het pas ontstaat, en de stand bij het besluit (onwaar,
/// of leeg).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct NogNiet {
    pub waarde: Value,
    pub stage: String,
}

/// De cel en het event waarin het proces een handeling laat vastleggen. Het
/// event heeft `zaak: volgt`; zijn `$external`-sleutels zijn uitkomsten van
/// de handeling of velden van haar formulier.
#[derive(Debug, Clone, Deserialize)]
pub struct Vastleggen {
    pub cel: String,
    pub stroom: String,
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
    pub tabel: InvoerVerwijzing,
    /// Per kolom van de tabel: onder welke naam ze in de parameter komt.
    /// Een kolom die hier niet staat, gaat niet mee.
    pub kolommen: BTreeMap<String, String>,
    /// Bronnen die per regel worden bevraagd.
    #[serde(default)]
    pub bronnen: Vec<RijBron>,
}

/// Een bron die per regel wordt bevraagd.
#[derive(Debug, Clone, Deserialize)]
pub struct RijBron {
    pub cel: String,
    /// Zonder url: de bron-cel draait in dezelfde runtime (intern transport).
    #[serde(default)]
    pub url: Option<String>,
    pub lexostatus: String,
    /// Per input van de bron: waar de waarde vandaan komt.
    pub invoer: BTreeMap<String, RijInvoer>,
    /// Per naam die de bron levert: onder welke kolomnaam ze in de regel komt.
    pub kolommen: BTreeMap<String, String>,
    /// Waarop de vertaling rust: de artikelen die de kolom bij de afnemer
    /// vragen en die de bron haar feit laten leveren, en een vaste waarde in
    /// de invoer (zie [`vertaalt`](RijBron::vertaalt)).
    #[serde(default)]
    pub grondslag: Vec<String>,
}

impl RijBron {
    /// Wat deze bron vertaalt: een kolom die bij de afnemer anders heet dan
    /// bij de bron, en een vaste waarde in de invoer. Leeg: niets.
    pub fn vertaalt(&self) -> Vec<String> {
        let mut uit: Vec<String> = self
            .kolommen
            .iter()
            .filter(|(b, a)| b != a)
            .map(|(b, a)| format!("{b} -> {a}"))
            .collect();
        uit.extend(self.invoer.iter().filter_map(|(n, i)| match i {
            RijInvoer::Waarde { waarde } => Some(format!("{n} = {waarde}")),
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
    Kolom { kolom: String },
    /// Een veld van een lexostatus van de zaak, of van een bron die het
    /// doorgeeft (een parameter of een extra veld).
    Eigen { lexostatus: String, veld: String },
    /// Een parameter uit de samenvoeging: de lexostatussen van de zaak en de
    /// synthese van het proces.
    Parameter { parameter: String },
    /// Een uitkomst van een regeling, uitgerekend met de samengevoegde
    /// parameters: de wet leidt de invoer af, zoals een peildatum uit een
    /// jaartal. Een keer per uitvoering, voor alle regels.
    Wet { regeling: String, uitkomst: String },
    /// Een vaste waarde.
    Waarde { waarde: Value },
}

/// Waar de invoer van een synthese-bron vandaan komt.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum BronInvoer {
    /// Een veld van een lexostatus van de zaak, of van een eerdere bron.
    Veld(InvoerVerwijzing),
    /// Een vaste waarde, zoals het orgaan waarvan het register wordt gevraagd.
    Waarde { waarde: Value },
}

impl BronInvoer {
    /// Het veld, als de invoer er een aanwijst.
    pub fn veld(&self) -> Option<&InvoerVerwijzing> {
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
    pub groep: Option<String>,
    pub uitleg: Option<String>,
}

/// Het portaalblok: in welke cel en welk event een indiening wordt, en welke
/// uitkomst de toets vraagt.
#[derive(Debug, Clone, Deserialize)]
pub struct Portaal {
    pub cel: String,
    pub stroom: String,
    pub event: String,
    pub toets: Toets,
    /// Wat het portaal aanbiedt: een uitkomst van het beleid van de actor,
    /// met optioneel de termijn die erbij getoond wordt.
    #[serde(default)]
    pub aanbod: Option<Aanbod>,
    #[serde(default)]
    pub formulier: Option<FormulierVerwijzing>,
}

/// Het aanbod van een portaal: een uitkomst van een regeling, uitgevoerd in
/// een run, en optioneel een tweede uitkomst van dezelfde regeling die de
/// termijn geeft.
#[derive(Debug, Clone, Deserialize)]
pub struct Aanbod {
    pub regeling: String,
    pub uitkomst: String,
    #[serde(default)]
    pub termijn: Option<String>,
    /// Een uitkomst van dezelfde regeling: de tijdvakken die het beleid
    /// aanbiedt, als het aanbod-artikel een tijdvak vraagt (de parameter met
    /// origin BELANGHEBBENDE en `rol: TIJDVAK`). Het portaal rekent
    /// haar uit in een run zonder parameters op de datum van vandaag.
    #[serde(default)]
    pub tijdvakken: Option<String>,
    /// Een uitkomst van dezelfde regeling: de eerste dag van een tijdvak, met
    /// het tijdvak als enige parameter. Het aanbod voor een tijdvak dat nog
    /// moet beginnen, peilt de registers op die dag; zonder op vandaag.
    #[serde(default)]
    pub begin: Option<String>,
    /// Een uitkomst van dezelfde regeling: de eerste dag waarop een aanvraag
    /// voor een tijdvak kan binnenkomen, met het tijdvak als enige parameter.
    /// Een loket voert geen ontvangst in van vóór die dag.
    #[serde(default)]
    pub openstelling: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Toets {
    pub lexostatus: String,
    pub regeling: String,
    pub uitkomst: String,
    /// Synthese per regel, zoals bij het besluit: een tabelveld van de
    /// toets-lexostatus wordt een array-parameter.
    #[serde(default)]
    pub rijen: Vec<RijenDefinitie>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormulierVerwijzing {
    pub pad: String,
    pub scherm: String,
}

/// Een lexostatus van een cel die het proces samenvoegt (synthese).
///
/// Een bron met `zaak: true` is een lexostatus van de zaak zelf, in de cel
/// waarin het proces vastlegt: het proces bevraagt haar met het zaakkenmerk,
/// en ze levert al haar parameters en extra velden. Elke andere bron noemt
/// haar invoer en haar parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct SyntheseBron {
    pub cel: String,
    /// Zonder url: de bron-cel draait in dezelfde runtime (intern transport).
    #[serde(default)]
    pub url: Option<String>,
    pub lexostatus: String,
    /// Een lexostatus van de zaak, met als enige input `zaakkenmerk`.
    #[serde(default)]
    pub zaak: bool,
    /// Per input van de bron: uit welk veld van een lexostatus van de zaak
    /// (bij de toets: de toets-lexostatus), van een eerdere bron, of een vaste
    /// waarde.
    #[serde(default)]
    pub invoer: BTreeMap<String, BronInvoer>,
    /// De parameters die deze bron levert, expliciet, met de naam bij de
    /// afnemer.
    #[serde(default)]
    pub parameters: Parameters,
    /// Velden uit het antwoord die geen parameter zijn, maar invoer voor een
    /// latere bron (bijvoorbeeld een naam bij een registratienummer).
    #[serde(default)]
    pub extra_velden: Vec<String>,
    /// Waarop de vertaling rust: de artikelen die het feit bij de afnemer
    /// onder zijn naam vragen en die de bron het laten leveren, en die een
    /// vaste waarde in de invoer dragen (zie [`vertaalt`](SyntheseBron::vertaalt)).
    #[serde(default)]
    pub grondslag: Vec<String>,
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
        uit.extend(self.invoer.iter().filter_map(|(n, i)| match i {
            BronInvoer::Waarde { waarde } => Some(format!("{n} = {waarde}")),
            BronInvoer::Veld(_) => None,
        }));
        uit
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvoerVerwijzing {
    pub lexostatus: String,
    pub veld: String,
}

impl CelDefinitie {
    /// Lees een celdefinitie uit tekst en valideer haar tegen het schema.
    pub fn parse(tekst: &str, bron: &str) -> Result<Self, Vec<String>> {
        laden::definitie(tekst, bron, Soort::Cel)
    }

    /// Laad `cel.yaml` uit de map van een cel.
    pub fn laad(map: &Path) -> Result<Self, Vec<String>> {
        laden::laad(&map.join(CEL_BESTAND), Self::parse)
    }
}

impl ProcesDefinitie {
    /// Lees een procesdefinitie uit tekst en valideer haar tegen het schema.
    pub fn parse(tekst: &str, bron: &str) -> Result<Self, Vec<String>> {
        laden::definitie(tekst, bron, Soort::Proces)
    }

    /// Laad `proces.yaml` uit de map van een proces.
    pub fn laad(map: &Path) -> Result<Self, Vec<String>> {
        laden::laad(&map.join(PROCES_BESTAND), Self::parse)
    }

    /// De rollen die een routegroep mogen gebruiken.
    pub fn rollen_met(&self, r: Routes) -> impl Iterator<Item = (&String, &RolDefinitie)> {
        self.rollen.iter().filter(move |(_, d)| d.mag(r))
    }

    /// De kanalen van de rollen die een routegroep mogen gebruiken, elk een
    /// keer, met hun id.
    pub fn kanalen_met(&self, r: Routes) -> Vec<(&str, &KanaalDefinitie)> {
        let mut uit: Vec<(&str, &KanaalDefinitie)> = Vec::new();
        for (_, rol) in self.rollen_met(r) {
            if let Some((id, k)) = self.kanalen.get_key_value(&rol.kanaal) {
                if !uit.iter().any(|(i, _)| *i == id) {
                    uit.push((id, k));
                }
            }
        }
        uit
    }

    /// De bronnen van de zaak (`zaak: true`), in de volgorde van de synthese.
    pub fn zaakbronnen(&self) -> impl Iterator<Item = &SyntheseBron> {
        self.synthese.iter().filter(|b| b.zaak)
    }

    /// De bronnen die geen lexostatus van de zaak zijn.
    pub fn andere_bronnen(&self) -> impl Iterator<Item = &SyntheseBron> {
        self.synthese.iter().filter(|b| !b.zaak)
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
        let mappen = celmappen(&fixtures().join("cellen")).unwrap();
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
                "test_register"
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
        assert_eq!(ids, ["test_afnemer_proces", "test_instantie_proces"]);
    }

    #[test]
    fn celdefinitie_valideert_tegen_het_schema() {
        let fout =
            CelDefinitie::parse("id: x\nrecording_actor: x\nstromen: []\n", "t").unwrap_err();
        assert!(fout.iter().any(|f| f.contains("lexostatussen")), "{fout:?}");
        assert!(fout.iter().any(|f| f.contains("/stromen")), "{fout:?}");
    }

    #[test]
    fn een_cel_heeft_geen_procesblokken() {
        let fout = CelDefinitie::parse(
            "id: a\nrecording_actor: a\nstromen: [s.yaml]\nlexostatussen: l.yaml\nrollen: {aanvrager: {kanaal: k, routes: [portaal]}}\n",
            "t",
        )
        .unwrap_err();
        assert!(fout.iter().any(|f| f.contains("rollen")), "{fout:?}");
    }

    #[test]
    fn synthese_bron_met_url() {
        let d = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthese:\n  - {cel: b, url: 'http://localhost:7172', lexostatus: l, invoer: {}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        assert_eq!(d.synthese[0].url.as_deref(), Some("http://localhost:7172"));
        assert!(d.portaal.is_none());
        assert!(d.voorbeelden.is_none());
    }

    /// De parameters van een bron: een lijst (dezelfde naam) of per naam bij
    /// de bron de naam bij de afnemer; een invoer is een veld of een vaste
    /// waarde.
    #[test]
    fn parameters_met_vertaling_en_een_vaste_invoer() {
        let d = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthese:\n  - {cel: b, lexostatus: l, invoer: {x: {lexostatus: e, veld: x}, orgaan: {waarde: raad}}, parameters: {is_ingeschreven_in_register: is_ingeschreven_raad}}\n  - {cel: c, lexostatus: m, invoer: {}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        let b = &d.synthese[0];
        assert_eq!(
            b.parameters.paren().collect::<Vec<_>>(),
            [("is_ingeschreven_in_register", "is_ingeschreven_raad")]
        );
        assert_eq!(
            b.parameters.iter().collect::<Vec<_>>(),
            ["is_ingeschreven_raad"]
        );
        assert!(matches!(&b.invoer["orgaan"], BronInvoer::Waarde { waarde } if waarde == "raad"));
        assert_eq!(b.invoer["x"].veld().unwrap().lexostatus, "e");
        assert_eq!(
            d.synthese[1].parameters.paren().collect::<Vec<_>>(),
            [("p", "p")]
        );
        assert!(d.synthese[1].parameters.vertaald().is_empty());
    }

    /// De tijdvakken en de stand bij besluit staan niet in de configuratie.
    #[test]
    fn keuzes_en_stand_bij_besluit_worden_geweigerd() {
        let fout = ProcesDefinitie::parse(
            "id: a\nactor: a\nportaal:\n  cel: a\n  stroom: s\n  event: e\n  toets: {lexostatus: l, regeling: r, uitkomst: u}\n  aanbod: {regeling: r, uitkomst: u, keuzes: {jaren_vanaf_nu: [0]}}\n",
            "t",
        )
        .unwrap_err();
        assert!(fout.iter().any(|f| f.contains("keuzes")), "{fout:?}");
        let fout = ProcesDefinitie::parse(
            "id: a\nactor: a\nbehandeling:\n  werkvoorraad: {cel: a, lexostatus: w}\n  handelingen:\n    - naam: b\n      uitkomsten: [u]\n      vastleggen: {cel: a, stroom: s, event: e}\n      stand_bij_besluit: {x: false}\n",
            "t",
        )
        .unwrap_err();
        assert!(
            fout.iter().any(|f| f.contains("stand_bij_besluit")),
            "{fout:?}"
        );
    }

    #[test]
    fn een_bron_van_de_zaak_noemt_geen_invoer_of_parameters() {
        let d = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthese:\n  - {cel: b, lexostatus: l, zaak: true}\n  - {cel: c, lexostatus: m, invoer: {x: {lexostatus: l, veld: x}}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        assert_eq!(d.zaakbronnen().count(), 1);
        assert_eq!(d.andere_bronnen().count(), 1);
        let fout = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthese:\n  - {cel: b, lexostatus: l, zaak: true, parameters: [p]}\n",
            "t",
        )
        .unwrap_err();
        assert!(fout.iter().any(|f| f.contains("/synthese/0")), "{fout:?}");
        let fout = ProcesDefinitie::parse(
            "id: a\nactor: a\nsynthese:\n  - {cel: b, lexostatus: l}\n",
            "t",
        )
        .unwrap_err();
        assert!(fout.iter().any(|f| f.contains("/synthese/0")), "{fout:?}");
    }

    /// Het besluitformulier staat niet in `proces.yaml`: het volgt uit de
    /// parameters met origin OORDEEL.
    #[test]
    fn een_besluitformulier_in_de_configuratie_wordt_geweigerd() {
        let fout = ProcesDefinitie::parse(
            "id: a\nactor: a\nbehandeling:\n  werkvoorraad: {cel: a, lexostatus: w}\n  handelingen:\n    - naam: b\n      uitkomsten: [u]\n      vastleggen: {cel: a, stroom: s, event: e}\n      formulier: [{parameter: p, label: P}]\n",
            "t",
        )
        .unwrap_err();
        assert!(fout.iter().any(|f| f.contains("formulier")), "{fout:?}");
    }

    #[test]
    fn voorbeelden_blok() {
        let d = ProcesDefinitie::parse(
            "id: a\nactor: a\nvoorbeelden:\n  inloggen: [login.json]\n  handelingen: {besluit: besluit.json}\n",
            "t",
        )
        .unwrap();
        let v = d.voorbeelden.unwrap();
        assert_eq!(v.inloggen, ["login.json"]);
        assert_eq!(v.aanvraag, None);
        assert_eq!(v.handelingen["besluit"], "besluit.json");
        let fout = ProcesDefinitie::parse(
            "id: a\nactor: a\nvoorbeelden:\n  inlog: [login.json]\n",
            "t",
        )
        .unwrap_err();
        assert!(fout.iter().any(|f| f.contains("inlog")), "{fout:?}");
    }

    #[test]
    fn map_zonder_cellen() {
        let dir = tempfile::tempdir().unwrap();
        assert!(celmappen(dir.path()).unwrap_err().contains("geen submap"));
        // Zonder processen: geen fout.
        assert!(procesmappen(dir.path()).unwrap().is_empty());
    }
}
