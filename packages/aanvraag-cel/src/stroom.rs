//! De stroomdefinitie: welke feiten de cel vastlegt, en het bouwen van een
//! gram uit wat binnenkomt.
//!
//! Een veld van een event bindt aan `$intake.<pad>` (wie en langs welke weg:
//! het ontvangstkanaal) of aan `$external.<pad>` (de inhoud zoals ingediend),
//! of is een constante van de stroom. Velden mogen genest zijn, en een
//! `$external`-waarde mag meer dan een veld voeden.

use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::schema::{self, Soort};

/// Een geladen stroomdefinitie.
#[derive(Debug, Clone)]
pub struct Stroom {
    pub id: String,
    pub recording_actor: String,
    pub chronicle: String,
    pub events: Vec<Event>,
    /// SHA-256 van het bestand zoals gelezen; gaat mee in elk gram.
    pub sha256: String,
    /// Het document zelf, voor `GET /api/stroom`.
    pub document: Value,
}

/// Een event uit een stroom.
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub name: String,
    pub intake: String,
    pub grondslag: Vec<String>,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub soort: Option<String>,
    /// De veldboom, in documentvolgorde (een YAML-mapping houdt die vast).
    pub fields: serde_yaml_ng::Mapping,
    #[serde(default)]
    pub niet_gereduceerd: Vec<NietGereduceerd>,
}

/// Een veld dat bewust door geen afleiding gelezen wordt, met de reden.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NietGereduceerd {
    pub veld: String,
    pub reden: String,
}

/// Waar de waarde van een veld vandaan komt.
#[derive(Debug, Clone, PartialEq)]
pub enum Binding {
    /// `$intake.<pad>`: het ontvangstkanaal.
    Intake(String),
    /// `$external.<pad>`: de inhoud zoals ingediend.
    External(String),
    /// Een vaste waarde van de stroom.
    Constante(Value),
}

/// Een blad van de veldboom: het pad in het gram en waaraan het bindt.
#[derive(Debug, Clone, PartialEq)]
pub struct Blad {
    /// Pad in het gram onder `fields`, bijvoorbeeld `inhoud.organen`.
    pub pad: String,
    pub binding: Binding,
}

/// Het vastgelegde gram (`schema/chronolex/v0.1.0/gram.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gram {
    pub kind: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soort: Option<String>,
    pub name: String,
    pub chronicle: String,
    pub recording_actor: String,
    pub grondslag: Vec<String>,
    pub op_moment: String,
    pub zaakkenmerk: String,
    pub stroom: StroomVerwijzing,
    pub fields: Map<String, Value>,
}

/// Welke stroomdefinitie een gram bouwde, en welke versie ervan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StroomVerwijzing {
    pub id: String,
    pub sha256: String,
}

impl Gram {
    /// Het gram als JSON-waarde.
    pub fn als_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Valideer het gram tegen `gram.json`.
    pub fn valideer(&self) -> Result<(), Vec<String>> {
        schema::valideer(Soort::Gram, &self.als_json())
    }

    /// De waarde op een pad onder `fields`.
    pub fn veld(&self, pad: &str) -> Option<&Value> {
        let mut delen = pad.split('.');
        let mut huidig = self.fields.get(delen.next()?)?;
        for deel in delen {
            huidig = huidig.as_object()?.get(deel)?;
        }
        Some(huidig)
    }
}

/// Lees een stroomdefinitie uit tekst. `bron` noemt het bestand in meldingen.
pub fn parse(tekst: &str, bron: &str) -> Result<Stroom, Vec<String>> {
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(tekst)
        .map_err(|e| vec![format!("{bron}: geen geldige YAML: {e}")])?;
    let document: Value = serde_json::to_value(&yaml).map_err(|e| vec![format!("{bron}: {e}")])?;
    schema::valideer(Soort::Stroom, &document).map_err(|fouten| {
        fouten
            .into_iter()
            .map(|f| format!("{bron}: {f}"))
            .collect::<Vec<_>>()
    })?;

    #[derive(Deserialize)]
    struct Ruw {
        #[serde(rename = "$id")]
        id: String,
        recording_actor: String,
        chronicle: String,
        events: Vec<Event>,
    }
    let ruw: Ruw = serde_yaml_ng::from_value(yaml).map_err(|e| vec![format!("{bron}: {e}")])?;
    Ok(Stroom {
        id: ruw.id,
        recording_actor: ruw.recording_actor,
        chronicle: ruw.chronicle,
        events: ruw.events,
        sha256: hex::encode(Sha256::digest(tekst.as_bytes())),
        document,
    })
}

/// Laad de stroomdefinities uit een bestand of uit alle `.yaml`-bestanden in
/// een map.
pub fn laad(pad: &Path) -> Result<Vec<Stroom>, Vec<String>> {
    let bestanden: Vec<std::path::PathBuf> = if pad.is_dir() {
        let mut v: Vec<_> = std::fs::read_dir(pad)
            .map_err(|e| vec![format!("{}: {e}", pad.display())])?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "yaml" || x == "yml"))
            .collect();
        v.sort();
        v
    } else {
        vec![pad.to_path_buf()]
    };
    let mut strommen = Vec::new();
    let mut fouten = Vec::new();
    for bestand in &bestanden {
        let bron = bestand.display().to_string();
        match std::fs::read_to_string(bestand) {
            Ok(tekst) => match parse(&tekst, &bron) {
                Ok(s) => strommen.push(s),
                Err(f) => fouten.extend(f),
            },
            Err(e) => fouten.push(format!("{bron}: {e}")),
        }
    }
    if strommen.is_empty() && fouten.is_empty() {
        fouten.push(format!("{}: geen stroomdefinitie gevonden", pad.display()));
    }
    if fouten.is_empty() {
        Ok(strommen)
    } else {
        Err(fouten)
    }
}

impl Stroom {
    /// Het event met deze naam.
    pub fn event(&self, naam: &str) -> Option<&Event> {
        self.events.iter().find(|e| e.name == naam)
    }
}

impl Event {
    /// De bladeren van de veldboom, in documentvolgorde.
    pub fn bladeren(&self) -> Vec<Blad> {
        use serde_yaml_ng::Value as Y;
        fn loop_(prefix: &str, velden: &serde_yaml_ng::Mapping, uit: &mut Vec<Blad>) {
            for (naam, waarde) in velden {
                let Some(naam) = naam.as_str() else { continue };
                let pad = if prefix.is_empty() {
                    naam.to_string()
                } else {
                    format!("{prefix}.{naam}")
                };
                let binding = match waarde {
                    Y::Mapping(kind) => {
                        loop_(&pad, kind, uit);
                        continue;
                    }
                    Y::String(tekst) => {
                        if let Some(r) = tekst.strip_prefix("$intake.") {
                            Binding::Intake(r.to_string())
                        } else if let Some(r) = tekst.strip_prefix("$external.") {
                            Binding::External(r.to_string())
                        } else {
                            Binding::Constante(Value::String(tekst.clone()))
                        }
                    }
                    ander => Binding::Constante(serde_json::to_value(ander).unwrap_or(Value::Null)),
                };
                uit.push(Blad { pad, binding });
            }
        }
        let mut uit = Vec::new();
        loop_("", &self.fields, &mut uit);
        uit
    }

    /// De velden van een gram van dit event als YAML, in de volgorde van de
    /// stroom (een JSON-object uit de kroniek is alfabetisch).
    pub fn geordend(&self, fields: &Map<String, Value>) -> serde_yaml_ng::Mapping {
        fn loop_(
            sjabloon: &serde_yaml_ng::Mapping,
            waarden: &Map<String, Value>,
        ) -> serde_yaml_ng::Mapping {
            let mut uit = serde_yaml_ng::Mapping::new();
            for (naam, sub) in sjabloon {
                let Some(naam) = naam.as_str() else { continue };
                let Some(waarde) = waarden.get(naam) else {
                    continue;
                };
                let geordend = match (sub, waarde) {
                    (serde_yaml_ng::Value::Mapping(s), Value::Object(w)) => {
                        serde_yaml_ng::Value::Mapping(loop_(s, w))
                    }
                    _ => serde_yaml_ng::to_value(waarde).unwrap_or(serde_yaml_ng::Value::Null),
                };
                uit.insert(serde_yaml_ng::Value::String(naam.to_string()), geordend);
            }
            uit
        }
        loop_(&self.fields, fields)
    }

    /// Of een pad een blad of een tak van de veldboom is.
    pub fn heeft_pad(&self, pad: &str) -> bool {
        self.bladeren()
            .iter()
            .any(|b| b.pad == pad || b.pad.starts_with(&format!("{pad}.")))
    }

    /// Of een pad een blad is (een veld met een eigen waarde, zoals een tabel).
    pub fn heeft_blad(&self, pad: &str) -> bool {
        self.bladeren().iter().any(|b| b.pad == pad)
    }

    /// De namen die een indiening onder `external` mag meegeven: het eerste
    /// deel van elk `$external.*`-pad.
    pub fn external_sleutels(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for blad in self.bladeren() {
            if let Binding::External(bronpad) = &blad.binding {
                let kop = bronpad.split('.').next().unwrap_or_default().to_string();
                if !v.contains(&kop) {
                    v.push(kop);
                }
            }
        }
        v
    }
}

/// Wat nodig is om een gram te bouwen, naast de stroom zelf.
pub struct Indiening<'a> {
    /// Het ontvangstkanaal: `kanaal` en wat de login meegeeft.
    pub intake: &'a Value,
    /// De inhoud zoals ingediend.
    pub external: &'a Map<String, Value>,
    pub op_moment: DateTime<FixedOffset>,
    pub zaakkenmerk: &'a str,
}

fn waarde_op<'v>(wortel: &'v Value, pad: &str) -> Option<&'v Value> {
    pad.split('.')
        .try_fold(wortel, |huidig, deel| huidig.as_object()?.get(deel))
}

/// Bouw een gram uit een indiening. Het gram houdt de vorm van de stroom:
/// een veld dat niet is ingevuld staat erin als null, want ook een
/// onvolledige indiening wordt vastgelegd. Een veld dat de stroom niet kent
/// wordt geweigerd: wat geen grondslag heeft, wordt niet vastgelegd.
pub fn bouw_gram(
    stroom: &Stroom,
    event: &Event,
    indiening: &Indiening<'_>,
) -> Result<Gram, String> {
    let bekend = event.external_sleutels();
    let mut onbekend: Vec<&str> = indiening
        .external
        .keys()
        .map(String::as_str)
        .filter(|k| !bekend.iter().any(|b| b == k))
        .collect();
    if !onbekend.is_empty() {
        onbekend.sort_unstable();
        return Err(format!(
            "onbekend veld {}: het event '{}' legt het niet vast",
            onbekend
                .iter()
                .map(|k| format!("'{k}'"))
                .collect::<Vec<_>>()
                .join(", "),
            event.name
        ));
    }

    let external = Value::Object(indiening.external.clone());
    let mut fields = Map::new();
    for blad in event.bladeren() {
        let waarde =
            match &blad.binding {
                Binding::Intake(bronpad) => waarde_op(indiening.intake, bronpad)
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "het ontvangstkanaal levert '$intake.{bronpad}' niet (veld '{}')",
                            blad.pad
                        )
                    })?,
                Binding::External(bronpad) => waarde_op(&external, bronpad)
                    .cloned()
                    .unwrap_or(Value::Null),
                Binding::Constante(w) => w.clone(),
            };
        zet(&mut fields, &blad.pad, waarde);
    }

    Ok(Gram {
        kind: "chronolexogram".to_string(),
        type_: event.type_.clone(),
        soort: event.soort.clone(),
        name: event.name.clone(),
        chronicle: stroom.chronicle.clone(),
        recording_actor: stroom.recording_actor.clone(),
        grondslag: event.grondslag.clone(),
        op_moment: indiening
            .op_moment
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        zaakkenmerk: indiening.zaakkenmerk.to_string(),
        stroom: StroomVerwijzing {
            id: stroom.id.clone(),
            sha256: stroom.sha256.clone(),
        },
        fields,
    })
}

fn zet(wortel: &mut Map<String, Value>, pad: &str, waarde: Value) {
    let delen: Vec<&str> = pad.split('.').collect();
    let mut huidig = wortel;
    for deel in &delen[..delen.len() - 1] {
        let volgende = huidig
            .entry((*deel).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !volgende.is_object() {
            *volgende = Value::Object(Map::new());
        }
        let Value::Object(m) = volgende else { return };
        huidig = m;
    }
    if let Some(laatste) = delen.last() {
        huidig.insert((*laatste).to_string(), waarde);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    const STROOM: &str = include_str!("../tests/fixtures/chronicles/test_aanvragen.yaml");

    fn moment() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2025-03-12T10:14:03+01:00").unwrap()
    }

    fn intake() -> Value {
        json!({"kanaal": "portaal", "eherkenning": {"kvk": "12345678", "persoon": "A. Tester", "machtiging": "volledig"}})
    }

    #[test]
    fn fixture_laadt_en_heeft_een_hash() {
        let s = parse(STROOM, "fixture").unwrap();
        assert_eq!(s.id, "test_aanvragen");
        assert_eq!(s.sha256.len(), 64);
        assert_eq!(s.events[0].grondslag, vec!["testregeling_aanvraag#1"]);
    }

    #[test]
    fn aanvraag_zonder_vaste_kern_faalt_op_het_schema() {
        let tekst = STROOM.replace("        dagtekening: $external.dagtekening\n", "");
        let fout = parse(&tekst, "t").unwrap_err();
        assert!(
            fout.iter().any(|f| f.contains("/events/0/fields/kern")),
            "{fout:?}"
        );
    }

    #[test]
    fn ongeldige_stroom_faalt_op_het_schema() {
        let fout = parse(
            "$id: x\nrecording_actor: y\nchronicle: z\nevents: []\n",
            "t",
        )
        .unwrap_err();
        assert!(fout[0].contains("/events"), "{fout:?}");
    }

    #[test]
    fn bladeren_in_documentvolgorde() {
        let s = parse(STROOM, "fixture").unwrap();
        let paden: Vec<String> = s.events[0].bladeren().into_iter().map(|b| b.pad).collect();
        assert_eq!(paden[0], "kern.aanvrager.naam");
        assert!(paden.contains(&"inhoud.organen".to_string()));
        assert!(s.events[0].heeft_pad("kern.ondertekend_via"));
        assert!(!s.events[0].heeft_blad("kern.ondertekend_via"));
        assert!(!s.events[0].heeft_pad("inhoud.bestaat_niet"));
        assert_eq!(
            s.events[0].external_sleutels(),
            vec![
                "naam",
                "adres",
                "dagtekening",
                "aanvraagjaar",
                "aanduiding",
                "registratie",
                "organen",
                "rekeningnummer"
            ]
        );
    }

    #[test]
    fn gram_bouwen_uit_intake_en_external() {
        let s = parse(STROOM, "fixture").unwrap();
        let external = json!({"naam": "Vereniging Voorbeeld", "aanvraagjaar": 2025, "organen": [{"orgaan": "raad", "zetels": 2}]});
        let gram = bouw_gram(
            &s,
            &s.events[0],
            &Indiening {
                intake: &intake(),
                external: external.as_object().unwrap(),
                op_moment: moment(),
                zaakkenmerk: "00000000-0000-4000-8000-000000000001",
            },
        )
        .unwrap();
        assert_eq!(gram.type_, "indiening");
        assert_eq!(gram.soort.as_deref(), Some("aanvraag"));
        assert_eq!(gram.op_moment, "2025-03-12T10:14:03+01:00");
        assert_eq!(
            gram.veld("kern.ondertekend_via.kvk_nummer"),
            Some(&json!("12345678"))
        );
        // Een $external-waarde voedt twee velden.
        assert_eq!(
            gram.veld("inhoud.naam"),
            Some(&json!("Vereniging Voorbeeld"))
        );
        assert_eq!(
            gram.veld("kern.aanvrager.naam"),
            Some(&json!("Vereniging Voorbeeld"))
        );
        // Een constante van de stroom.
        assert_eq!(
            gram.veld("kern.gevraagde_beschikking"),
            Some(&json!("testbeschikking, testregeling artikel 1"))
        );
        // Niet ingevuld: vastgelegd als null, de vorm blijft.
        assert_eq!(gram.veld("inhoud.aanduiding"), Some(&Value::Null));
        gram.valideer().unwrap();
    }

    #[test]
    fn onbekend_veld_wordt_geweigerd() {
        let s = parse(STROOM, "fixture").unwrap();
        let external = json!({"schoenmaat": 44});
        let fout = bouw_gram(
            &s,
            &s.events[0],
            &Indiening {
                intake: &intake(),
                external: external.as_object().unwrap(),
                op_moment: moment(),
                zaakkenmerk: "00000000-0000-4000-8000-000000000001",
            },
        )
        .unwrap_err();
        assert!(fout.contains("'schoenmaat'"), "{fout}");
    }

    #[test]
    fn ontbrekende_intake_is_een_fout() {
        let s = parse(STROOM, "fixture").unwrap();
        let external = Map::new();
        let fout = bouw_gram(
            &s,
            &s.events[0],
            &Indiening {
                intake: &json!({"kanaal": "portaal"}),
                external: &external,
                op_moment: moment(),
                zaakkenmerk: "00000000-0000-4000-8000-000000000001",
            },
        )
        .unwrap_err();
        assert!(fout.contains("$intake.eherkenning.kvk"), "{fout}");
    }
}
