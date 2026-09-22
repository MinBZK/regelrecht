//! Configuratie uit de omgeving, en het laden van een gecontroleerde cel.

use std::path::{Path, PathBuf};

use regelrecht_engine::LawExecutionService;

use crate::formulier::{self, Formulier};
use crate::reductie::{self, CelConfig, Portaal};
use crate::stroom::{self, Event, Stroom};
use crate::{controle, regelingen};

/// Standaardpoort, binnen 7100-7300.
pub const STANDAARD_POORT: u16 = 7170;

#[derive(Debug, Clone)]
pub struct Config {
    /// Map met de regelingen (het corpus).
    pub regulation_path: PathBuf,
    /// Een stroombestand, of een map met stroombestanden.
    pub chronicles_path: PathBuf,
    /// De celconfiguratie met lexostatus-definities en het portaalblok.
    pub cell_config_path: PathBuf,
    /// Map voor de kronieken (`<chronicle>.jsonl`).
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
        let port = match std::env::var("AANVRAAG_CEL_PORT") {
            Ok(v) => v
                .parse()
                .map_err(|_| format!("AANVRAAG_CEL_PORT '{v}' is geen poortnummer"))?,
            Err(_) => STANDAARD_POORT,
        };
        Ok(Self {
            regulation_path: pad("REGULATION_PATH")?,
            chronicles_path: pad("CHRONICLES_PATH")?,
            cell_config_path: pad("CELL_CONFIG_PATH")?,
            data_dir: pad("DATA_DIR")?,
            port,
        })
    }
}

/// Een geladen cel die de controles bij het opstarten doorstond.
pub struct Cel {
    pub strommen: Vec<Stroom>,
    pub config: CelConfig,
    pub service: LawExecutionService,
    pub formulier: Option<Formulier>,
}

impl Cel {
    /// Laad stroom, celconfiguratie, regelingen en formulier, en controleer
    /// ze. Elke fout komt terug, niet alleen de eerste.
    pub fn laad(config: &Config) -> Result<Self, Vec<String>> {
        let mut fouten = Vec::new();
        let strommen = stroom::laad(&config.chronicles_path)
            .map_err(|f| fouten.extend(f))
            .ok();
        let cel = reductie::laad(&config.cell_config_path)
            .map_err(|f| fouten.extend(f))
            .ok();
        let service = regelingen::laad(&config.regulation_path)
            .map_err(|f| fouten.extend(f))
            .ok();
        let (Some(strommen), Some(cel), Some(service)) = (strommen, cel, service) else {
            return Err(fouten);
        };
        controle::controleer(&strommen, &cel, &service)?;
        let formulier = match cel.portaal.as_ref().and_then(|p| p.formulier.as_ref()) {
            Some(f) => {
                let basis = config.cell_config_path.parent().unwrap_or(Path::new("."));
                Some(formulier::laad(&basis.join(&f.pad), &f.scherm).map_err(|e| vec![e])?)
            }
            None => None,
        };
        Ok(Self {
            strommen,
            config: cel,
            service,
            formulier,
        })
    }

    /// Het portaalblok. Bestaat altijd: de controle eist het.
    pub fn portaal(&self) -> Option<&Portaal> {
        self.config.portaal.as_ref()
    }

    /// De stroom en het event waarin het portaal vastlegt.
    pub fn portaal_event(&self) -> Option<(&Stroom, &Event)> {
        let p = self.portaal()?;
        let s = self.strommen.iter().find(|s| s.id == p.stroom)?;
        Some((s, s.event(&p.event)?))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    pub(crate) fn fixture_config(data_dir: &Path) -> Config {
        let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        Config {
            regulation_path: f.join("regulation"),
            chronicles_path: f.join("chronicles"),
            cell_config_path: f.join("cel/lexostatussen.yaml"),
            data_dir: data_dir.to_path_buf(),
            port: STANDAARD_POORT,
        }
    }

    #[test]
    fn fixture_cel_laadt() {
        let dir = tempfile::tempdir().unwrap();
        let cel = Cel::laad(&fixture_config(dir.path())).unwrap();
        assert_eq!(cel.portaal_event().unwrap().1.name, "aanvraag_ontvangen");
        assert!(cel.formulier.is_some());
    }

    #[test]
    fn ontbrekende_bestanden_worden_allemaal_gemeld() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = fixture_config(dir.path());
        c.chronicles_path = dir.path().join("geen.yaml");
        c.cell_config_path = dir.path().join("geen_cel.yaml");
        let fouten = Cel::laad(&c).err().unwrap();
        assert_eq!(fouten.len(), 2, "{fouten:?}");
    }
}
