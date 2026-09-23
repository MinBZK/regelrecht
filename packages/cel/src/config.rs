//! Configuratie: de omgeving van de runtime, en de celdefinitie (`cel.yaml`).
//!
//! Een cel is een map onder `CELLS_PATH` met een `cel.yaml`. Paden daarin
//! zijn relatief aan die map.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::schema::{self, Soort};

/// Standaardpoort, binnen 7100-7300.
pub const STANDAARD_POORT: u16 = 7170;

/// De naam van het bestand dat van een map een cel maakt.
pub const CEL_BESTAND: &str = "cel.yaml";

/// De omgeving van de runtime.
#[derive(Debug, Clone)]
pub struct Config {
    /// Map met een submap per cel, elk met een `cel.yaml`.
    pub cells_path: PathBuf,
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
            regulation_path: pad("REGULATION_PATH")?,
            data_dir: pad("DATA_DIR")?,
            port,
        })
    }
}

/// Een celdefinitie (`schema/chronolex/v0.1.0/cel.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct CelDefinitie {
    pub id: String,
    pub recording_actor: String,
    pub stromen: Vec<String>,
    pub lexostatussen: String,
    /// Wie er inlogt, en hoe. Zonder rollen is er geen login.
    #[serde(default)]
    pub rollen: Rollen,
    #[serde(default)]
    pub portaal: Option<Portaal>,
    #[serde(default)]
    pub synthese: Vec<SyntheseBron>,
    #[serde(default)]
    pub behandeling: Option<Behandeling>,
    #[serde(default)]
    pub startstand: Option<String>,
}

/// De rollen van een cel, elk met een (nagebootste) login.
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

/// Wat de behandelaar in de cel doet: een werkvoorraad en een besluit.
#[derive(Debug, Clone, Deserialize)]
pub struct Behandeling {
    /// Een lijst-lexostatus van deze cel.
    pub werkvoorraad: String,
    pub besluit: BesluitDefinitie,
}

/// Het besluit op een zaak: de uitkomsten van een artikel, en per parameter
/// de bron (zie [`crate::besluit`]).
#[derive(Debug, Clone, Deserialize)]
pub struct BesluitDefinitie {
    /// Leeg in `cel.yaml`: de runtime vult haar bij het laden met de regeling
    /// van de beschikking waarvoor de cel bevoegd is (zie
    /// [`crate::besluit::beschikkingen_van`]).
    #[serde(default)]
    pub regeling: String,
    pub uitkomsten: Vec<String>,
    /// Eigen lexostatussen met als enige input `zaakkenmerk`.
    pub lexostatussen: Vec<String>,
    /// De oordelen van de behandelaar.
    #[serde(default)]
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

/// Het event waarin de cel het genomen besluit vastlegt. Het event heeft
/// `zaak: volgt` en een stage, en zijn `$external`-sleutels zijn precies de
/// uitkomsten van het besluit.
#[derive(Debug, Clone, Deserialize)]
pub struct Vastleggen {
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
    /// Het tabelveld van een eigen lexostatus (een extra veld of parameter).
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
    /// Een veld van een eigen lexostatus (een parameter of een extra veld).
    Eigen {
        lexostatus: String,
        veld: String,
        #[serde(default)]
        als: Option<Omzetting>,
    },
    /// Een parameter uit de samenvoeging: de eigen lexostatussen en de
    /// synthese van de cel.
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

/// Een veld van het besluitformulier: een parameter met een label.
#[derive(Debug, Clone, Deserialize)]
pub struct Oordeel {
    pub parameter: String,
    pub label: String,
    #[serde(default)]
    pub groep: Option<String>,
    #[serde(default)]
    pub uitleg: Option<String>,
}

/// Het portaalblok: welk event een indiening wordt en welke uitkomst de
/// toets vraagt.
#[derive(Debug, Clone, Deserialize)]
pub struct Portaal {
    pub stroom: String,
    pub event: String,
    pub toets: Toets,
    /// De uitkomst die zegt of wie inlogt namens de organisatie mag handelen.
    #[serde(default)]
    pub mandaat: Option<UitkomstVerwijzing>,
    /// De uitkomst die de uiterste indieningsdatum geeft; alleen getoond.
    #[serde(default)]
    pub termijn: Option<UitkomstVerwijzing>,
    #[serde(default)]
    pub formulier: Option<FormulierVerwijzing>,
}

/// Een uitkomst van een regeling.
#[derive(Debug, Clone, Deserialize)]
pub struct UitkomstVerwijzing {
    pub regeling: String,
    pub uitkomst: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Toets {
    pub lexostatus: String,
    pub regeling: String,
    pub uitkomst: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormulierVerwijzing {
    pub pad: String,
    pub scherm: String,
}

/// Een lexostatus van een andere cel die de toets samenvoegt met de eigen.
#[derive(Debug, Clone, Deserialize)]
pub struct SyntheseBron {
    pub cel: String,
    /// Zonder url: de bron-cel draait in dezelfde runtime (intern transport).
    #[serde(default)]
    pub url: Option<String>,
    pub lexostatus: String,
    /// Per input van de bron: uit welk veld van de eigen lexostatus.
    pub invoer: BTreeMap<String, InvoerVerwijzing>,
    /// De parameters die deze bron levert, expliciet.
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
        let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(tekst)
            .map_err(|e| vec![format!("{bron}: geen geldige YAML: {e}")])?;
        let document: Value =
            serde_json::to_value(&yaml).map_err(|e| vec![format!("{bron}: {e}")])?;
        schema::valideer(Soort::Cel, &document).map_err(|f| {
            f.into_iter()
                .map(|f| format!("{bron}: {f}"))
                .collect::<Vec<_>>()
        })?;
        serde_json::from_value(document).map_err(|e| vec![format!("{bron}: {e}")])
    }

    /// Laad `cel.yaml` uit de map van een cel.
    pub fn laad(map: &Path) -> Result<Self, Vec<String>> {
        let pad = map.join(CEL_BESTAND);
        let bron = pad.display().to_string();
        let tekst = std::fs::read_to_string(&pad).map_err(|e| vec![format!("{bron}: {e}")])?;
        Self::parse(&tekst, &bron)
    }
}

/// De mappen onder `CELLS_PATH` met een `cel.yaml`, gesorteerd.
pub fn celmappen(cells_path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut mappen: Vec<PathBuf> = std::fs::read_dir(cells_path)
        .map_err(|e| format!("{}: {e}", cells_path.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join(CEL_BESTAND).is_file())
        .collect();
    mappen.sort();
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
    fn celdefinitie_valideert_tegen_het_schema() {
        let fout =
            CelDefinitie::parse("id: x\nrecording_actor: x\nstromen: []\n", "t").unwrap_err();
        assert!(fout.iter().any(|f| f.contains("lexostatussen")), "{fout:?}");
        assert!(fout.iter().any(|f| f.contains("/stromen")), "{fout:?}");
    }

    #[test]
    fn synthese_bron_met_url() {
        let d = CelDefinitie::parse(
            "id: a\nrecording_actor: a\nstromen: [s.yaml]\nlexostatussen: l.yaml\nsynthese:\n  - {cel: b, url: 'http://localhost:7172', lexostatus: l, invoer: {}, parameters: [p]}\n",
            "t",
        )
        .unwrap();
        assert_eq!(d.synthese[0].url.as_deref(), Some("http://localhost:7172"));
        assert!(d.portaal.is_none());
    }

    #[test]
    fn map_zonder_cellen() {
        let dir = tempfile::tempdir().unwrap();
        assert!(celmappen(dir.path()).unwrap_err().contains("geen submap"));
    }
}
