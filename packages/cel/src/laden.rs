//! Het lezen van configuratie van schijf, op een plek: een bestand met zijn
//! naam in elke melding, een YAML-document gevalideerd tegen zijn schema, en
//! de bestanden en submappen van een map. Een fout bij het lezen van een map
//! wordt nooit overgeslagen: een bestand dat er niet te lezen staat, is een
//! melding en geen stilte.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::schema::{self, Soort};

/// De tekst van een bestand, met de naam waaronder meldingen het noemen.
pub fn lees(pad: &Path) -> Result<(String, String), String> {
    let bron = pad.display().to_string();
    let tekst = std::fs::read_to_string(pad).map_err(|e| format!("{bron}: {e}"))?;
    Ok((tekst, bron))
}

/// Lees een bestand en zet het om met `parse(tekst, bron)`.
pub fn laad<T>(
    pad: &Path,
    parse: impl FnOnce(&str, &str) -> Result<T, Vec<String>>,
) -> Result<T, Vec<String>> {
    let (tekst, bron) = lees(pad).map_err(|e| vec![e])?;
    parse(&tekst, &bron)
}

/// Een YAML-document, gevalideerd tegen zijn schema: de YAML-boom (die de
/// volgorde van het document houdt) en hetzelfde als JSON. Elke melding noemt
/// `bron`.
pub fn yaml_document(
    tekst: &str,
    bron: &str,
    soort: Soort,
) -> Result<(serde_yaml_ng::Value, Value), Vec<String>> {
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(tekst)
        .map_err(|e| vec![format!("{bron}: geen geldige YAML: {e}")])?;
    let document: Value = serde_json::to_value(&yaml).map_err(|e| vec![format!("{bron}: {e}")])?;
    schema::valideer(soort, &document).map_err(|f| {
        f.into_iter()
            .map(|f| format!("{bron}: {f}"))
            .collect::<Vec<_>>()
    })?;
    Ok((yaml, document))
}

/// Lees een YAML-definitie, valideer haar tegen haar schema en zet haar om.
pub fn definitie<T: DeserializeOwned>(
    tekst: &str,
    bron: &str,
    soort: Soort,
) -> Result<T, Vec<String>> {
    let (_, document) = yaml_document(tekst, bron, soort)?;
    serde_json::from_value(document).map_err(|e| vec![format!("{bron}: {e}")])
}

/// De paden in een map, gesorteerd. Een item dat niet te lezen is, is een
/// fout.
fn inhoud(map: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paden = Vec::new();
    for item in std::fs::read_dir(map).map_err(|e| format!("{}: {e}", map.display()))? {
        paden.push(item.map_err(|e| format!("{}: {e}", map.display()))?.path());
    }
    paden.sort();
    Ok(paden)
}

/// De `.yaml`- en `.yml`-bestanden in een map, gesorteerd.
pub fn yaml_bestanden(map: &Path) -> Result<Vec<PathBuf>, String> {
    Ok(inhoud(map)?
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "yaml" || x == "yml"))
        .collect())
}

/// De submappen van een map met een `bestand` erin, gesorteerd.
pub fn mappen_met(map: &Path, bestand: &str) -> Result<Vec<PathBuf>, String> {
    Ok(inhoud(map)?
        .into_iter()
        .filter(|p| p.join(bestand).is_file())
        .collect())
}
