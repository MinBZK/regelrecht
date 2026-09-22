//! De JSON-schema's uit `schema/chronolex/v0.1.0/`, ingebakken bij het
//! bouwen. De bestanden zijn de bron; deze module compileert ze eenmaal.

use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;

const STREAM: &str = include_str!("../../../schema/chronolex/v0.1.0/stream.json");
const LEXOSTATUS: &str = include_str!("../../../schema/chronolex/v0.1.0/lexostatus.json");
const GRAM: &str = include_str!("../../../schema/chronolex/v0.1.0/gram.json");
const CEL: &str = include_str!("../../../schema/chronolex/v0.1.0/cel.json");

/// Welk van de drie schema's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Soort {
    /// Een stroomdefinitie (`stream.json`).
    Stroom,
    /// De lexostatus-definities van een cel (`lexostatus.json`).
    Lexostatus,
    /// Een celdefinitie, `cel.yaml` (`cel.json`).
    Cel,
    /// Een vastgelegd gram (`gram.json`).
    Gram,
}

fn compileer(bron: &str, naam: &str) -> Result<Validator, String> {
    let schema: Value = serde_json::from_str(bron)
        .map_err(|e| format!("ingebakken schema {naam} is geen geldige JSON: {e}"))?;
    Validator::new(&schema).map_err(|e| format!("ingebakken schema {naam} compileert niet: {e}"))
}

static STREAM_V: LazyLock<Result<Validator, String>> =
    LazyLock::new(|| compileer(STREAM, "stream.json"));
static LEXOSTATUS_V: LazyLock<Result<Validator, String>> =
    LazyLock::new(|| compileer(LEXOSTATUS, "lexostatus.json"));
static GRAM_V: LazyLock<Result<Validator, String>> = LazyLock::new(|| compileer(GRAM, "gram.json"));
static CEL_V: LazyLock<Result<Validator, String>> = LazyLock::new(|| compileer(CEL, "cel.json"));

/// Valideer een document tegen een van de schema's. Bij een fout: elke
/// schending als `<pad>: <melding>`.
pub fn valideer(soort: Soort, doc: &Value) -> Result<(), Vec<String>> {
    let validator = match soort {
        Soort::Stroom => &*STREAM_V,
        Soort::Lexostatus => &*LEXOSTATUS_V,
        Soort::Gram => &*GRAM_V,
        Soort::Cel => &*CEL_V,
    }
    .as_ref()
    .map_err(|e| vec![e.clone()])?;
    let fouten: Vec<String> = validator
        .iter_errors(doc)
        .map(|e| {
            let pad = e.instance_path().to_string();
            if pad.is_empty() {
                e.to_string()
            } else {
                format!("{pad}: {e}")
            }
        })
        .collect();
    if fouten.is_empty() {
        Ok(())
    } else {
        Err(fouten)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn alle_schemas_compileren() {
        for (naam, v) in [
            ("stream", &*STREAM_V),
            ("lexostatus", &*LEXOSTATUS_V),
            ("gram", &*GRAM_V),
            ("cel", &*CEL_V),
        ] {
            assert!(v.is_ok(), "{naam}: {:?}", v.as_ref().err());
        }
    }

    #[test]
    fn schending_noemt_het_pad() {
        let doc = serde_json::json!({"$id": "x", "recording_actor": "y", "chronicle": "z", "events": [{"name": "Fout"}]});
        let fouten = valideer(Soort::Stroom, &doc).unwrap_err();
        assert!(
            fouten.iter().any(|f| f.starts_with("/events/0")),
            "{fouten:?}"
        );
    }
}
