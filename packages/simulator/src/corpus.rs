//! Regelingen uit het corpus opzoeken.
//!
//! Een cel noemt haar wetten bij `$id`. In het corpus is dat de naam van de map
//! (`corpus/regulation/<land>/<soort>/<id>/<valid_from>.yaml`); elk bestand in
//! die map is één versie van dezelfde regeling. De simulator laadt ze allemaal,
//! zodat de engine zelf op `op_moment` de juiste versie kiest.

use crate::error::{Result, SimulatorError};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Waar het regelingcorpus staat.
///
/// `REGULATION_PATH` wint, anders `corpus/regulation` naast deze crate. Zelfde
/// afspraak als de integratietests van de engine, zodat een checkout op een
/// andere plek met één env-var werkt.
pub fn regulation_root() -> PathBuf {
    std::env::var("REGULATION_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("corpus")
                .join("regulation")
        })
}

/// Waar de testregelingen van deze crate staan.
///
/// Een tweede wortel náást het corpus, met opzet klein: een scenario dat een
/// eigenschap van de *opstelling* moet aantonen heeft soms een regeling nodig die
/// in het corpus niet thuishoort. Tier 3 is daar het voorbeeld van — een wet met
/// een `source.regulation` die een organisatie aanwijst in plaats van een
/// regeling — en zo'n wet hoort niet in het echte corpus te gaan staan omdat een
/// testopstelling haar nodig heeft.
///
/// Het corpus gaat vóór: een naam die daar bestaat, wordt daar geladen, dus een
/// fixture kan geen echte regeling overschaduwen.
fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("regulation")
}

/// Alleen de `$id` uit een regelingdocument; de rest laat de engine parseren.
#[derive(Deserialize)]
struct RegulationId {
    #[serde(rename = "$id")]
    id: String,
}

/// Lees alle versies van één regeling als YAML-teksten, op pad gesorteerd.
///
/// Zoekt eerst in het corpus en daarna in de testregelingen van deze crate
/// ([`fixture_root`]). Faalt als geen van beide de map heeft, als ze leeg is, of
/// als er een document in staat waarvan de `$id` niet met de mapnaam
/// overeenkomt — dan zou de cel iets anders laden dan ze denkt te laden, en dat
/// is geen fout om stil te slikken.
pub(crate) fn regulation_versions(root: &Path, regulation: &str) -> Result<Vec<String>> {
    let found = versions_under(root, regulation)?;
    if !found.is_empty() {
        return Ok(found);
    }

    let fixtures = fixture_root();
    let found = versions_under(&fixtures, regulation)?;
    if !found.is_empty() {
        return Ok(found);
    }

    Err(SimulatorError::RegulationNotFound {
        regulation: regulation.to_string(),
        root: root.to_path_buf(),
    })
}

/// Alle versies van één regeling onder één wortel; leeg als ze er niet staat.
fn versions_under(root: &Path, regulation: &str) -> Result<Vec<String>> {
    let mut documents: Vec<(PathBuf, String)> = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_dir() && e.file_name() == regulation)
    {
        for file in std::fs::read_dir(entry.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = file.path();
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "yaml") {
                continue;
            }
            let text =
                std::fs::read_to_string(&path).map_err(|source| SimulatorError::FileRead {
                    path: path.clone(),
                    source,
                })?;
            let parsed: RegulationId = serde_yaml_ng::from_str(&text).map_err(|source| {
                SimulatorError::RegulationParse {
                    path: path.clone(),
                    source,
                }
            })?;
            if parsed.id != regulation {
                return Err(SimulatorError::RegulationIdMismatch {
                    path,
                    expected: regulation.to_string(),
                    found: parsed.id,
                });
            }
            documents.push((path, text));
        }
    }

    documents.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(documents.into_iter().map(|(_, text)| text).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vindt_alle_versies_van_een_regeling() {
        let versions = regulation_versions(&regulation_root(), "regeling_standaardpremie")
            .unwrap_or_else(|e| panic!("standaardpremie moet in het corpus staan: {e}"));
        assert!(
            versions.len() >= 2,
            "standaardpremie heeft meerdere jaarversies, kreeg {}",
            versions.len()
        );
    }

    #[test]
    fn onbekende_regeling_geeft_nette_fout() {
        let err = regulation_versions(&regulation_root(), "wet_op_de_niet_bestaande_dingen")
            .expect_err("een niet-bestaande regeling mag niet slagen");
        assert!(
            matches!(err, SimulatorError::RegulationNotFound { .. }),
            "verwachtte RegulationNotFound, kreeg {err}"
        );
    }
}
