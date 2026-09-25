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
    /// Wie er inlogt, en hoe. Zonder rollen is er geen login.
    #[serde(default)]
    pub rollen: Rollen,
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
    /// Logins voor de nep-eHerkenning, elk `{kvk, persoon}`.
    #[serde(default)]
    pub inloggen: Vec<String>,
    /// Een aanvraag: `{external: {...}}`.
    #[serde(default)]
    pub aanvraag: Option<String>,
    /// Een besluitformulier: `{formulier: {...}}`.
    #[serde(default)]
    pub besluit: Option<String>,
}

/// De rollen van een proces, elk met een (nagebootste) login.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Rollen {
    /// Wie via het portaal indient.
    #[serde(default)]
    pub aanvrager: Option<AanvragerLogin>,
    /// Wie de werkvoorraad, de zaken en het besluit ziet.
    #[serde(default)]
    pub behandelaar: Option<BehandelaarLogin>,
}

impl Rollen {
    pub fn is_leeg(&self) -> bool {
        self.aanvrager.is_none() && self.behandelaar.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AanvragerLogin {
    Eherkenning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BehandelaarLogin {
    Medewerker,
}

/// Wat de behandelaar in het proces doet: een werkvoorraad en een besluit.
#[derive(Debug, Clone, Deserialize)]
pub struct Behandeling {
    /// Een lijst-lexostatus van de cel van het proces.
    pub werkvoorraad: LexostatusVerwijzing,
    pub besluit: BesluitDefinitie,
}

/// Een lexostatus van een cel.
#[derive(Debug, Clone, Deserialize)]
pub struct LexostatusVerwijzing {
    pub cel: String,
    pub lexostatus: String,
}

/// Het besluit op een zaak: de uitkomsten van een artikel, en per parameter
/// de bron (zie [`crate::besluit`]).
#[derive(Debug, Clone, Deserialize)]
pub struct BesluitDefinitie {
    /// Leeg in `proces.yaml`: de runtime vult haar bij het laden met de
    /// regeling van de beschikking waarvoor de actor van het proces bevoegd
    /// is (zie [`crate::besluit::beschikkingen_van`]).
    #[serde(default)]
    pub regeling: String,
    pub uitkomsten: Vec<String>,
    /// De oordelen van de behandelaar: niet in `proces.yaml`, maar bij het
    /// laden afgeleid uit de parameters van het besluit met origin `OORDEEL`
    /// (zie [`crate::origin::oordelen`]).
    #[serde(skip)]
    pub formulier: Vec<Oordeel>,
    /// Feiten die pas na het besluit ontstaan, met hun stand bij het besluit.
    #[serde(default)]
    pub stand_bij_besluit: BTreeMap<String, Value>,
    /// Synthese per regel: een tabelveld wordt een array-parameter.
    #[serde(default)]
    pub rijen: Vec<RijenDefinitie>,
    /// Waar het besluit als gram wordt vastgelegd.
    #[serde(default)]
    pub vastleggen: Option<Vastleggen>,
}

/// De cel en het event waarin het proces het genomen besluit laat
/// vastleggen. Het event heeft `zaak: volgt` en een stage, en zijn
/// `$external`-sleutels zijn precies de uitkomsten van het besluit.
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
}

/// Waar de invoer van een bron per regel vandaan komt.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RijInvoer {
    /// Een kolom van de regel zelf, zoals die na de kolomnamen heet.
    Kolom {
        kolom: String,
        #[serde(default)]
        als: Option<Omzetting>,
    },
    /// Een veld van een lexostatus van de zaak, of van een bron die het
    /// doorgeeft (een parameter of een extra veld).
    Eigen {
        lexostatus: String,
        veld: String,
        #[serde(default)]
        als: Option<Omzetting>,
    },
    /// Een parameter uit de samenvoeging: de lexostatussen van de zaak en de
    /// synthese van het proces.
    Parameter {
        parameter: String,
        #[serde(default)]
        als: Option<Omzetting>,
    },
}

/// Een omzetting van een waarde voor ze als invoer meegaat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Omzetting {
    /// Van een jaartal naar de datum 1 januari van dat jaar. De tegenhanger
    /// van de afleiding `jaar_van`.
    EersteDagVanHetJaar,
}

impl RijInvoer {
    pub fn omzetting(&self) -> Option<Omzetting> {
        match self {
            RijInvoer::Kolom { als, .. }
            | RijInvoer::Eigen { als, .. }
            | RijInvoer::Parameter { als, .. } => *als,
        }
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
    /// Welke tijdvakken het portaal aanbiedt, als het aanbod-artikel een
    /// tijdvak vraagt (de parameter met origin BELANGHEBBENDE en grondslag
    /// Awb 4:2 lid 1). Voorlopig configuratie, niet uit de wet.
    #[serde(default)]
    pub keuzes: Option<Keuzes>,
}

/// De tijdvakken die het portaal aanbiedt.
#[derive(Debug, Clone, Deserialize)]
pub struct Keuzes {
    /// Jaartallen, geteld vanaf het jaar van vandaag: `[0, 1]` is dit jaar en
    /// het volgende.
    pub jaren_vanaf_nu: Vec<i64>,
}

impl Keuzes {
    /// De waarden van het tijdvak in een jaar.
    pub fn waarden(&self, jaar: i64) -> Vec<i64> {
        self.jaren_vanaf_nu.iter().map(|d| jaar + d).collect()
    }
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
    /// (bij de toets: de toets-lexostatus), of van een eerdere bron.
    #[serde(default)]
    pub invoer: BTreeMap<String, InvoerVerwijzing>,
    /// De parameters die deze bron levert, expliciet.
    #[serde(default)]
    pub parameters: Vec<String>,
    /// Velden uit het antwoord die geen parameter zijn, maar invoer voor een
    /// latere bron (bijvoorbeeld een naam bij een registratienummer).
    #[serde(default)]
    pub extra_velden: Vec<String>,
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
            "id: a\nrecording_actor: a\nstromen: [s.yaml]\nlexostatussen: l.yaml\nrollen: {aanvrager: eherkenning}\n",
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
            "id: a\nactor: a\nbehandeling:\n  werkvoorraad: {cel: a, lexostatus: w}\n  besluit:\n    uitkomsten: [u]\n    formulier: [{parameter: p, label: P}]\n",
            "t",
        )
        .unwrap_err();
        assert!(fout.iter().any(|f| f.contains("formulier")), "{fout:?}");
    }

    #[test]
    fn voorbeelden_blok() {
        let d = ProcesDefinitie::parse(
            "id: a\nactor: a\nvoorbeelden:\n  inloggen: [login.json]\n  besluit: besluit.json\n",
            "t",
        )
        .unwrap();
        let v = d.voorbeelden.unwrap();
        assert_eq!(v.inloggen, ["login.json"]);
        assert_eq!(v.aanvraag, None);
        assert_eq!(v.besluit.as_deref(), Some("besluit.json"));
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
