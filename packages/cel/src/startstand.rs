//! De startstand: grammen die een cel bij het starten in een lege kroniek
//! zet.
//!
//! Een regel van het JSONL-bestand noemt de stroom, het event (`name`),
//! `op_moment`, `herkomst: startstand` en `fields`. Een `zaakkenmerk` staat
//! erin als het event een zaak heeft (`zaak: opent` of `volgt`), en alleen
//! dan; de startstand verzint er geen. De rest (type, soort, grondslag, kroniek, actor en de hash
//! van de stroom) volgt uit de stroom, zodat een startstand niet veroudert als
//! de stroom verandert. De velden moeten precies die van het event zijn.
//!
//! Een startstand-gram is geplaatst, niet berekend: er is geen engine-trace
//! bij. Daarom draagt het `herkomst: startstand`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::laden;
use crate::stroom::{op_pad, Event, Gram, Stroom, StroomVerwijzing};

/// De enige herkomst die een regel van de startstand mag hebben.
pub const HERKOMST: &str = "startstand";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Regel {
    stroom: String,
    name: String,
    op_moment: String,
    herkomst: String,
    #[serde(default)]
    zaakkenmerk: Option<String>,
    fields: Map<String, Value>,
}

/// Lees de startstand en bouw de grammen. Elke fout noemt de regel.
pub fn laad(pad: &Path, strommen: &[Stroom]) -> Result<Vec<Gram>, Vec<String>> {
    laden::laad(pad, |tekst, bron| parse(tekst, bron, strommen))
}

/// Bouw de grammen uit de tekst van een startstand.
pub fn parse(tekst: &str, bron: &str, strommen: &[Stroom]) -> Result<Vec<Gram>, Vec<String>> {
    let mut grammen = Vec::new();
    let mut fouten = Vec::new();
    for (i, regel) in tekst.lines().enumerate() {
        if regel.trim().is_empty() {
            continue;
        }
        match bouw(regel, strommen) {
            Ok(g) => grammen.push(g),
            Err(f) => fouten.push(format!("{bron} regel {}: {f}", i + 1)),
        }
    }
    if fouten.is_empty() {
        Ok(grammen)
    } else {
        Err(fouten)
    }
}

fn bouw(tekst: &str, strommen: &[Stroom]) -> Result<Gram, String> {
    let regel: Regel = serde_json::from_str(tekst).map_err(|e| e.to_string())?;
    if regel.herkomst != HERKOMST {
        return Err(format!(
            "herkomst is '{}', een startstand heeft herkomst '{HERKOMST}'",
            regel.herkomst
        ));
    }
    let stroom = strommen
        .iter()
        .find(|s| s.id == regel.stroom)
        .ok_or_else(|| format!("stroom '{}' is geen stroom van deze cel", regel.stroom))?;
    let event = stroom
        .event(&regel.name)
        .ok_or_else(|| format!("stroom '{}' heeft geen event '{}'", stroom.id, regel.name))?;
    velden_passen(event, &regel.fields, "")?;
    event
        .zaak
        .toets_kenmerk(&event.name, regel.zaakkenmerk.as_deref())?;
    let gram = Gram {
        kind: "chronolexogram".into(),
        type_: event.type_.clone(),
        soort: event.soort.clone(),
        stage: event.stage.clone(),
        name: event.name.clone(),
        chronicle: stroom.chronicle.clone(),
        recording_actor: stroom.recording_actor.clone(),
        grondslag: event.grondslag.clone(),
        legal_character: None,
        decision_type: None,
        regulation: None,
        regulation_valid_from: None,
        competent_authority: None,
        op_moment: regel.op_moment,
        zaak: event.zaak,
        zaakkenmerk: regel.zaakkenmerk,
        stroom: StroomVerwijzing {
            id: stroom.id.clone(),
            sha256: stroom.sha256.clone(),
        },
        herkomst: Some(HERKOMST.into()),
        fields: regel.fields,
        inputs: BTreeMap::new(),
        receipt: None,
    };
    gram.valideer()
        .map_err(|f| format!("gram valideert niet: {}", f.join("; ")))?;
    Ok(gram)
}

/// De velden zijn precies die van het event: geen onbekend veld, en elk
/// blad van het event staat erin (desnoods als null).
fn velden_passen(event: &Event, velden: &Map<String, Value>, prefix: &str) -> Result<(), String> {
    for (naam, waarde) in velden {
        let pad = format!("{prefix}{naam}");
        if !event.heeft_pad(&pad) {
            return Err(format!("event '{}' heeft geen veld '{pad}'", event.name));
        }
        if !event.heeft_blad(&pad) {
            let Value::Object(sub) = waarde else {
                return Err(format!("veld '{pad}' verwacht velden eronder"));
            };
            velden_passen(event, sub, &format!("{pad}."))?;
        }
    }
    if prefix.is_empty() {
        for blad in event.bladeren() {
            if op_pad(velden, &blad.pad).is_none() {
                return Err(format!(
                    "veld '{}' van event '{}' ontbreekt",
                    blad.pad, event.name
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const STROOM: &str = include_str!("../tests/fixtures/chronicles/test_registers.yaml");
    const STARTSTAND: &str = include_str!("../tests/fixtures/cellen/register/startstand.jsonl");

    fn strommen() -> Vec<Stroom> {
        vec![crate::stroom::parse(STROOM, "stroom").unwrap()]
    }

    fn fout(regel: &str) -> String {
        parse(regel, "s", &strommen()).unwrap_err().join("; ")
    }

    #[test]
    fn fixture_startstand() {
        let grammen = parse(STARTSTAND, "s", &strommen()).unwrap();
        assert_eq!(grammen.len(), 4);
        for g in &grammen {
            assert_eq!(g.herkomst.as_deref(), Some("startstand"));
            assert_eq!(g.type_, "decretogram");
            assert_eq!(g.chronicle, "test_register");
            g.valideer().unwrap();
        }
        assert_eq!(grammen[0].grondslag, ["testregeling_register#1"]);
    }

    #[test]
    fn regel_zonder_herkomst_startstand() {
        let r = r#"{"stroom": "test_registers", "name": "aanduiding_geschrapt", "op_moment": "2024-01-10T09:00:00+01:00", "herkomst": "engine", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(fout(r).contains("herkomst 'startstand'"), "{}", fout(r));
    }

    #[test]
    fn onbekend_en_ontbrekend_veld() {
        let r = r#"{"stroom": "test_registers", "name": "aanduiding_geschrapt", "op_moment": "2024-01-10T09:00:00+01:00", "herkomst": "startstand", "fields": {"aanduiding": "X", "orgaan": "raad", "kleur": 1}}"#;
        assert!(fout(r).contains("geen veld 'kleur'"), "{}", fout(r));
        let r = r#"{"stroom": "test_registers", "name": "aanduiding_geschrapt", "op_moment": "2024-01-10T09:00:00+01:00", "herkomst": "startstand", "fields": {"aanduiding": "X"}}"#;
        assert!(fout(r).contains("'orgaan' van event"), "{}", fout(r));
    }

    #[test]
    fn onbekend_event_en_ongeldig_moment() {
        let r = r#"{"stroom": "test_registers", "name": "bestaat_niet", "op_moment": "2024-01-10T09:00:00+01:00", "herkomst": "startstand", "fields": {}}"#;
        assert!(fout(r).contains("geen event 'bestaat_niet'"));
        let r = r#"{"stroom": "test_registers", "name": "aanduiding_geschrapt", "op_moment": "gisteren", "herkomst": "startstand", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(fout(r).contains("op_moment"), "{}", fout(r));
        assert!(fout(r).starts_with("s regel 1: "));
    }

    #[test]
    fn geen_zaak_geen_zaakkenmerk() {
        // De startstand verzint geen zaakkenmerk.
        for g in parse(STARTSTAND, "s", &strommen()).unwrap() {
            assert_eq!(g.zaakkenmerk, None);
            assert!(g.als_json().get("zaakkenmerk").is_none());
            assert!(g.als_json().get("zaak").is_none());
        }
        let r = r#"{"stroom": "test_registers", "name": "aanduiding_geschrapt", "op_moment": "2024-01-10T09:00:00+01:00", "herkomst": "startstand", "zaakkenmerk": "00000000-0000-4000-8000-000000000001", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(
            fout(r).contains("heeft geen zaak (zaak: geen)"),
            "{}",
            fout(r)
        );
    }

    #[test]
    fn zaakkenmerk_bij_een_event_met_een_zaak() {
        for zaak in ["opent", "volgt"] {
            let stroom = STROOM.replace(
                "  - name: aanduiding_geschrapt\n",
                &format!("  - name: aanduiding_geschrapt\n    zaak: {zaak}\n"),
            );
            let strommen = vec![crate::stroom::parse(&stroom, "stroom").unwrap()];
            let zonder = r#"{"stroom": "test_registers", "name": "aanduiding_geschrapt", "op_moment": "2024-01-10T09:00:00+01:00", "herkomst": "startstand", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
            let f = parse(zonder, "s", &strommen).unwrap_err().join("; ");
            assert!(f.contains("het zaakkenmerk ontbreekt"), "{f}");
            let met = zonder.replace(
                r#""fields""#,
                r#""zaakkenmerk": "00000000-0000-4000-8000-000000000001", "fields""#,
            );
            let g = parse(&met, "s", &strommen).unwrap().remove(0);
            assert_eq!(
                g.zaakkenmerk.as_deref(),
                Some("00000000-0000-4000-8000-000000000001")
            );
            assert_eq!(g.als_json()["zaak"], zaak);
            g.valideer().unwrap();
        }
    }
}
