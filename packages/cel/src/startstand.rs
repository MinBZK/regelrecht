//! De startstand: grammen die een cel bij het starten in een lege kroniek
//! zet.
//!
//! Een regel van het JSONL-bestand noemt de stroom, het event (`name`),
//! `op_moment`, `herkomst: startstand` en `fields`, en zo nodig `id` en
//! `verwijst` (naar een eerder gram van de startstand, met de namen van het
//! event). Zonder `id` krijgt het gram een vast id uit de regel zelf, zodat
//! elke lading hetzelfde id geeft. De rest (type, soort, grondslag, kroniek, actor en de hash
//! van de stroom) volgt uit de stroom, zodat een startstand niet veroudert als
//! de stroom verandert. De velden moeten precies die van het event zijn.
//!
//! Een startstand-gram is geplaatst, niet berekend: er is geen engine-trace
//! bij. Daarom draagt het `herkomst: startstand`.
//!
//! Het `op_moment` van een regel is met de hand gezet: wanneer het feit
//! rechtens geldt of plaatsvond, zoals de dag van een besluit of van een
//! vaststelling. `vastgelegd_op` staat er niet in: dat is de laadtijd, het
//! moment waarop de runtime de startstand in de lege kroniek zet
//! ([`geplaatst`]). Zo zegt een startstand nooit dat de cel iets eerder wist
//! dan ze het had.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::datum;
use crate::gram::{op_pad, Gram, StroomVerwijzing};
use crate::laden;
use crate::stroom::{Event, Stroom};

/// De enige herkomst die een regel van de startstand mag hebben.
pub const HERKOMST: &str = "initial_state";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Regel {
    stream: String,
    name: String,
    effective_at: String,
    provenance: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    refers_to: BTreeMap<String, String>,
    fields: Map<String, Value>,
}

/// Lees de startstand en bouw de grammen. Elke fout noemt de regel.
pub fn laad(path: &Path, streams: &[Stroom]) -> Result<Vec<Gram>, Vec<String>> {
    laden::laad(path, |tekst, source| parse(tekst, source, streams))
}

/// Bouw de grammen uit de tekst van een startstand.
pub fn parse(tekst: &str, source: &str, streams: &[Stroom]) -> Result<Vec<Gram>, Vec<String>> {
    let mut grams = Vec::new();
    let mut fouten = Vec::new();
    for (i, regel) in tekst.lines().enumerate() {
        if regel.trim().is_empty() {
            continue;
        }
        match bouw(regel, streams) {
            Ok(g) => grams.push(g),
            Err(f) => fouten.push(format!("{source} regel {}: {f}", i + 1)),
        }
    }
    if fouten.is_empty() {
        Ok(grams)
    } else {
        Err(fouten)
    }
}

fn bouw(tekst: &str, streams: &[Stroom]) -> Result<Gram, String> {
    let regel: Regel = serde_json::from_str(tekst).map_err(|e| e.to_string())?;
    if regel.provenance != HERKOMST {
        return Err(format!(
            "herkomst is '{}', een startstand heeft herkomst '{HERKOMST}'",
            regel.provenance
        ));
    }
    let stream = streams
        .iter()
        .find(|s| s.id == regel.stream)
        .ok_or_else(|| format!("stroom '{}' is geen stroom van deze cel", regel.stream))?;
    let event = stream
        .event(&regel.name)
        .ok_or_else(|| format!("stroom '{}' heeft geen event '{}'", stream.id, regel.name))?;
    velden_passen(event, &regel.fields, "")?;
    crate::stroom::toets_verwijzingen(event, &regel.refers_to)?;
    let id = regel.id.clone().unwrap_or_else(|| {
        crate::gram::vast_id(&format!(
            "urn:regelrecht:cel:startstand:{}:{}",
            stream.id,
            tekst.trim()
        ))
    });
    let gram = Gram {
        kind: "chronolexogram".into(),
        id,
        type_: event.type_.clone(),
        subtype: event.subtype.clone(),
        stage: event.stage.clone(),
        name: event.name.clone(),
        chronicle: stream.chronicle.clone(),
        recording_actor: stream.recording_actor.clone(),
        legal_basis: event.legal_basis.clone(),
        legal_character: None,
        decision_type: None,
        regulation: None,
        regulation_valid_from: None,
        competent_authority: None,
        acting_actor: None,
        effective_at: regel.effective_at,
        effective_at_legal_basis: None,
        // De laadtijd komt bij het plaatsen, zie `geplaatst`.
        recorded_at: String::new(),
        refers_to: regel.refers_to,
        stream: StroomVerwijzing {
            id: stream.id.clone(),
            sha256: stream.sha256.clone(),
        },
        provenance: Some(HERKOMST.into()),
        fields: regel.fields,
        inputs: BTreeMap::new(),
        receipt: None,
        tijden: Default::default(),
        root: None,
    };
    // Valideren kan al: met het op_moment op de plaats van de laadtijd.
    Gram {
        recorded_at: gram.effective_at.clone(),
        ..gram.clone()
    }
    .valideer()
    .map_err(|f| format!("gram valideert niet: {}", f.join("; ")))?;
    Ok(gram)
}

/// De startstand zoals de runtime haar in een lege kroniek zet: elk gram
/// met de laadtijd als `vastgelegd_op`. Een regel met een `op_moment` na de
/// laadtijd wordt geweigerd, zoals bij elk ander gram: wat nog moet gebeuren,
/// is geen feit. Een besluit met werking vanaf een latere dag (een schrapping
/// met ingang van volgend jaar) staat er op de dag waarop het genomen is; de
/// dag van ingang is dan een veld van het gram.
pub fn geplaatst(grams: &[Gram], laadtijd: &DateTime<FixedOffset>) -> Result<Vec<Gram>, String> {
    let op = datum::als_op_moment(laadtijd);
    let mut uit = Vec::with_capacity(grams.len());
    for g in grams {
        if g.moment()? > *laadtijd {
            return Err(format!(
                "initial_state: '{}' heeft op_moment {}, na de laadtijd ({op}); wat nog moet gebeuren, wordt niet vastgelegd",
                g.name, g.effective_at
            ));
        }
        uit.push(Gram {
            recorded_at: op.clone(),
            ..g.clone()
        });
    }
    Ok(uit)
}

/// De velden zijn precies die van het event: geen onbekend veld, en elk
/// blad van het event staat erin (desnoods als null).
fn velden_passen(event: &Event, fields: &Map<String, Value>, prefix: &str) -> Result<(), String> {
    for (name, value) in fields {
        let path = format!("{prefix}{name}");
        if !event.heeft_pad(&path) {
            return Err(format!("event '{}' heeft geen veld '{path}'", event.name));
        }
        if !event.heeft_blad(&path) {
            let Value::Object(sub) = value else {
                return Err(format!("veld '{path}' verwacht velden eronder"));
            };
            velden_passen(event, sub, &format!("{path}."))?;
        }
    }
    if prefix.is_empty() {
        for blad in event.bladeren() {
            if op_pad(fields, &blad.path).is_none() {
                return Err(format!(
                    "veld '{}' van event '{}' ontbreekt",
                    blad.path, event.name
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
    const STARTSTAND: &str = include_str!("../tests/fixtures/cells/register/initial_state.jsonl");

    fn streams() -> Vec<Stroom> {
        vec![crate::stroom::parse(STROOM, "stroom").unwrap()]
    }

    fn error(regel: &str) -> String {
        parse(regel, "s", &streams()).unwrap_err().join("; ")
    }

    #[test]
    fn fixture_startstand() {
        let grams = parse(STARTSTAND, "s", &streams()).unwrap();
        assert_eq!(grams.len(), 4);
        let laadtijd = datum::moment("2025-03-12T10:14:03+01:00").unwrap();
        for (g, geplaatst) in grams.iter().zip(geplaatst(&grams, &laadtijd).unwrap()) {
            assert_eq!(g.provenance.as_deref(), Some("initial_state"));
            assert_eq!(g.type_, "decretogram");
            assert_eq!(g.chronicle, "test_register");
            // Twee tijden: het op_moment van de regel, de laadtijd als
            // vastgelegd_op.
            assert_eq!(geplaatst.effective_at, g.effective_at);
            assert_eq!(geplaatst.recorded_at, "2025-03-12T10:14:03+01:00");
            geplaatst.valideer().unwrap();
        }
        assert_eq!(grams[0].legal_basis, ["testregeling_register#1"]);
    }

    /// Een regel met een op_moment na de laadtijd wordt niet geplaatst.
    #[test]
    fn een_startstand_uit_de_toekomst_wordt_geweigerd() {
        let grams = parse(STARTSTAND, "s", &streams()).unwrap();
        let eerder = datum::moment("2024-02-01T00:00:00+01:00").unwrap();
        let f = geplaatst(&grams, &eerder).unwrap_err();
        assert!(f.contains("na de laadtijd"), "{f}");
    }

    #[test]
    fn een_startstand_zet_geen_vastgelegd_op() {
        // Dat is de laadtijd: een startstand zegt niet wanneer de cel het
        // wist.
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "recorded_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(error(r).contains("recorded_at"), "{}", error(r));
    }

    #[test]
    fn regel_zonder_herkomst_startstand() {
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "engine", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(
            error(r).contains("herkomst 'initial_state'"),
            "{}",
            error(r)
        );
    }

    #[test]
    fn onbekend_en_ontbrekend_veld() {
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "X", "orgaan": "raad", "kleur": 1}}"#;
        assert!(error(r).contains("geen veld 'kleur'"), "{}", error(r));
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "fields": {"aanduiding": "X"}}"#;
        assert!(error(r).contains("'orgaan' van event"), "{}", error(r));
    }

    #[test]
    fn onbekend_event_en_ongeldig_moment() {
        let r = r#"{"stream": "test_registers", "name": "bestaat_niet", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "fields": {}}"#;
        assert!(error(r).contains("geen event 'bestaat_niet'"));
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "gisteren", "provenance": "initial_state", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(error(r).contains("effective_at"), "{}", error(r));
        assert!(error(r).starts_with("s regel 1: "));
    }

    /// Een regel zonder id krijgt een vast id uit de regel zelf; een regel
    /// mag een id en verwijzingen noemen, met de namen van het event.
    #[test]
    fn id_en_verwijzingen() {
        let a = parse(STARTSTAND, "s", &streams()).unwrap();
        let b = parse(STARTSTAND, "s", &streams()).unwrap();
        assert_eq!(a[0].id, b[0].id, "elke lading hetzelfde id");
        assert_ne!(a[0].id, a[1].id);
        assert!(a[0].refers_to.is_empty());
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "refers_to": {"zaak": "00000000-0000-4000-8000-000000000001"}, "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        assert!(
            error(r).contains("verwijst niet met 'zaak'"),
            "{}",
            error(r)
        );
        let r = r#"{"stream": "test_registers", "name": "aanduiding_geschrapt", "effective_at": "2024-01-10T09:00:00+01:00", "provenance": "initial_state", "id": "00000000-0000-4000-8000-00000000000a", "fields": {"aanduiding": "X", "orgaan": "raad"}}"#;
        let g = parse(r, "s", &streams()).unwrap().remove(0);
        assert_eq!(g.id, "00000000-0000-4000-8000-00000000000a");
        let laadtijd = datum::moment("2025-03-12T10:14:03+01:00").unwrap();
        geplaatst(&[g], &laadtijd).unwrap()[0].valideer().unwrap();
    }
}
