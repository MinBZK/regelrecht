//! De JSON-schema's uit `schema/chronolex/v0.2.0/`, ingebakken bij het
//! bouwen. De bestanden zijn de bron; deze module compileert ze eenmaal.

use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;

const STREAM: &str = include_str!("../../../schema/chronolex/v0.2.0/stream.json");
const LEXOSTATUS: &str = include_str!("../../../schema/chronolex/v0.2.0/lexostatus.json");
const GRAM: &str = include_str!("../../../schema/chronolex/v0.2.0/gram.json");
const CEL: &str = include_str!("../../../schema/chronolex/v0.2.0/cel.json");
const PROCES: &str = include_str!("../../../schema/chronolex/v0.2.0/proces.json");

/// Welk van de schema's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Soort {
    /// Een stroomdefinitie (`stream.json`).
    Stroom,
    /// De lexostatus-definities van een cel (`lexostatus.json`).
    Lexostatus,
    /// Een celdefinitie, `cel.yaml` (`cel.json`).
    Cel,
    /// Een procesdefinitie, `proces.yaml` (`proces.json`).
    Proces,
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
static PROCES_V: LazyLock<Result<Validator, String>> =
    LazyLock::new(|| compileer(PROCES, "proces.json"));

/// Valideer een document tegen een van de schema's. Bij een fout: elke
/// schending als `<pad>: <melding>`.
pub fn valideer(soort: Soort, doc: &Value) -> Result<(), Vec<String>> {
    let validator = match soort {
        Soort::Stroom => &*STREAM_V,
        Soort::Lexostatus => &*LEXOSTATUS_V,
        Soort::Gram => &*GRAM_V,
        Soort::Cel => &*CEL_V,
        Soort::Proces => &*PROCES_V,
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
            ("proces", &*PROCES_V),
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

    fn gram() -> Value {
        serde_json::json!({
            "kind": "chronolexogram", "id": "01900000-0000-7000-8000-000000000001",
            "type": "decretogram", "name": "x",
            "chronicle": "k", "recording_actor": "a", "grondslag": ["r#1"],
            "op_moment": "2025-03-12T10:14:03+01:00",
            "vastgelegd_op": "2025-03-12T10:14:03+01:00",
            "stroom": {"id": "s", "sha256": "0".repeat(64)}, "fields": {}
        })
    }

    /// Een gram heeft een id (een uuid) en verwijst met een naam naar een
    /// ander id; een zaak- of besluitkenmerk kent v0.2.0 niet meer.
    #[test]
    fn gram_id_en_verwijzingen() {
        let mut g = gram();
        valideer(Soort::Gram, &g).unwrap();
        g["verwijst"] = serde_json::json!({"op_aanvraag": "00000000-0000-4000-8000-000000000001"});
        valideer(Soort::Gram, &g).unwrap();
        g["verwijst"] = serde_json::json!({"op_aanvraag": "g-001"});
        assert!(valideer(Soort::Gram, &g).is_err());
        let mut z = gram();
        z["zaakkenmerk"] = "00000000-0000-4000-8000-000000000001".into();
        assert!(valideer(Soort::Gram, &z).is_err());
        let mut z = gram();
        z.as_object_mut().unwrap().remove("id");
        assert!(valideer(Soort::Gram, &z).is_err());
    }

    #[test]
    fn stage_op_een_decretogram_indiening_of_handeling() {
        let mut g = gram();
        g["type"] = "indiening".into();
        g["soort"] = "melding".into();
        g["stage"] = "AANVRAAG".into();
        valideer(Soort::Gram, &g).unwrap();
        g["type"] = "decretogram".into();
        g["stage"] = "BESLUIT".into();
        valideer(Soort::Gram, &g).unwrap();
        // Een executogram heeft geen stage.
        g["type"] = "executogram".into();
        assert!(valideer(Soort::Gram, &g).is_err());
    }

    #[test]
    fn stroom_verwijst_met_naam_en_naar() {
        let stroom = |verwijst: Value| {
            serde_json::json!({"$id": "s", "recording_actor": "a", "chronicle": "k", "events": [{
                "name": "x", "intake": "besluit", "grondslag": ["r#1"],
                "type": "decretogram", "verwijst": verwijst, "fields": {"a": "$external.a"}
            }]})
        };
        for naar in [
            serde_json::json!("algemene_wet_bestuursrecht#4:1"),
            serde_json::json!("aanvraag_ontvangen"),
            serde_json::json!({"stage": "BESLUIT"}),
        ] {
            valideer(
                Soort::Stroom,
                &stroom(serde_json::json!({"op_aanvraag": {"naar": naar, "verplicht": true}})),
            )
            .unwrap();
        }
        assert!(valideer(
            Soort::Stroom,
            &stroom(serde_json::json!({"op_aanvraag": {}}))
        )
        .is_err());
        let mut z = stroom(serde_json::json!({"besluit": {"naar": {"stage": "BESLUIT"}}}));
        z["events"][0]["zaak"] = "volgt".into();
        let fouten = valideer(Soort::Stroom, &z).unwrap_err();
        assert!(fouten.iter().any(|f| f.contains("zaak")), "{fouten:?}");
    }
}
