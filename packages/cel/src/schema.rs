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

    fn gram(zaak: Option<&str>, zaakkenmerk: Option<&str>) -> Value {
        let mut g = serde_json::json!({
            "kind": "chronolexogram", "type": "decretogram", "name": "x",
            "chronicle": "k", "recording_actor": "a", "grondslag": ["r#1"],
            "op_moment": "2025-03-12T10:14:03+01:00",
            "stroom": {"id": "s", "sha256": "0".repeat(64)}, "fields": {}
        });
        if let Some(z) = zaak {
            g["zaak"] = z.into();
        }
        if let Some(k) = zaakkenmerk {
            g["zaakkenmerk"] = k.into();
        }
        g
    }

    #[test]
    fn gram_zaakkenmerk_alleen_bij_een_zaak() {
        const Z: &str = "00000000-0000-4000-8000-000000000001";
        for zaak in ["opent", "volgt"] {
            valideer(Soort::Gram, &gram(Some(zaak), Some(Z))).unwrap();
            let fouten = valideer(Soort::Gram, &gram(Some(zaak), None)).unwrap_err();
            assert!(
                fouten.iter().any(|f| f.contains("zaakkenmerk")),
                "{fouten:?}"
            );
        }
        for zaak in [Some("geen"), None] {
            valideer(Soort::Gram, &gram(zaak, None)).unwrap();
            assert!(valideer(Soort::Gram, &gram(zaak, Some(Z))).is_err());
        }
        assert!(valideer(Soort::Gram, &gram(Some("misschien"), None)).is_err());
    }

    #[test]
    fn stroom_zaak_is_opent_volgt_of_geen() {
        let stroom = |zaak: &str| {
            serde_json::json!({"$id": "s", "recording_actor": "a", "chronicle": "k", "events": [{
                "name": "x", "intake": "besluit", "grondslag": ["r#1"],
                "type": "decretogram", "zaak": zaak, "fields": {"a": "$external.a"}
            }]})
        };
        for zaak in ["opent", "volgt", "geen"] {
            valideer(Soort::Stroom, &stroom(zaak)).unwrap();
        }
        let fouten = valideer(Soort::Stroom, &stroom("misschien")).unwrap_err();
        assert!(
            fouten.iter().any(|f| f.starts_with("/events/0/zaak")),
            "{fouten:?}"
        );
    }
}
