//! De kroniekstore van een cel: de feiten die de cel zélf bijhoudt.
//!
//! Een kroniekstroom is een tijdgeordende groepering van vastleggingen over
//! hetzelfde soort onderwerp, gesleuteld op één veld (meestal een BSN). Elke
//! vastlegging draagt het moment waarop het feit feit werd — de as waarop een
//! reductie ordent (RFC-022 §4.1).
//!
//! De store is bewust *niet* publiek bereikbaar vanaf een cel: zie
//! [`crate::cell::Cell`].

use crate::error::{Result, SimulatorError};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Eén vastlegging in een kroniekstroom.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChronicleEvent {
    /// Het moment waarop dit feit in deze cel feit werd.
    pub op_moment: NaiveDate,
    /// De vastgelegde velden. Veldnamen komen overeen met de `input`-namen van
    /// de regelingen van de cel; de engine matcht hoofdletterongevoelig.
    pub fields: BTreeMap<String, Value>,
}

/// Een tijdgeordende groepering van vastleggingen, gesleuteld op één veld.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChronicleStream {
    /// Naam van de stroom; dient tevens als naam van de databron in de engine.
    pub stream: String,
    /// Het veld waarop vastleggingen gegroepeerd worden (bijvoorbeeld `bsn`).
    pub key: String,
    /// De vastleggingen, in willekeurige volgorde; de store ordent zelf.
    #[serde(default)]
    pub events: Vec<ChronicleEvent>,
}

/// De feiten van één cel.
///
/// Alleen te bevragen via een reductie van de cel die haar bezit — er is geen
/// weg van buiten naar binnen (invariant: geen gedeelde state).
#[derive(Debug, Default)]
pub struct ChronicleStore {
    streams: Vec<ChronicleStream>,
}

/// Eén stroom, teruggebracht tot de feiten die op een moment bekend waren.
pub(crate) struct ReducedStream {
    /// Naam van de stroom.
    pub(crate) stream: String,
    /// Het sleutelveld van de stroom.
    pub(crate) key: String,
    /// Per sleutelwaarde één samengevoegd record.
    pub(crate) records: Vec<BTreeMap<String, Value>>,
}

impl ChronicleStore {
    /// Bouw een store uit de stromen van een celconfiguratie.
    ///
    /// Faalt als een vastlegging het sleutelveld van haar stroom mist: dan valt
    /// niet vast te stellen over welk onderwerp het feit gaat. Faalt ook op twee
    /// stromen met dezelfde naam: die naam is tevens de naam van de databron in
    /// de engine, en daar zou de tweede de eerste stil schaduwen.
    pub(crate) fn from_streams(cell: &str, streams: Vec<ChronicleStream>) -> Result<Self> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for stream in &streams {
            if !seen.insert(stream.stream.as_str()) {
                return Err(SimulatorError::DuplicateStream {
                    cell: cell.to_string(),
                    stream: stream.stream.clone(),
                });
            }
            for event in &stream.events {
                if !event
                    .fields
                    .keys()
                    .any(|f| f.eq_ignore_ascii_case(&stream.key))
                {
                    return Err(SimulatorError::ChronicleEventWithoutKey {
                        stream: stream.stream.clone(),
                        key: stream.key.clone(),
                        op_moment: event.op_moment.to_string(),
                    });
                }
            }
        }
        Ok(Self { streams })
    }

    /// De feiten zoals ze op `op_moment` in deze cel bekend waren.
    ///
    /// Dit is de tijdreductie: vastleggingen ná `op_moment` bestaan voor deze
    /// vraag niet, en van de rest wint per sleutel en per veld de laatste
    /// vastlegging. Een vraag over een moment in het verleden levert dus het
    /// beeld van toen, niet het beeld van nu.
    ///
    /// Twee vastleggingen op hetzelfde moment kunnen niet op de tijdas uit
    /// elkaar gehouden worden. De volgorde in de configuratie beslist dan, en de
    /// laatste wint — `sort_by_key` is stabiel, dus dat is een vastgelegde
    /// eigenschap en geen toeval. De dag is in deze versie de fijnste korrel;
    /// wie twee vastleggingen op één dag wil ordenen, heeft een fijnere tijdas
    /// nodig en niet een andere sorteersleutel.
    pub(crate) fn reduce_to(&self, op_moment: NaiveDate) -> Vec<ReducedStream> {
        self.streams
            .iter()
            .map(|stream| {
                let mut events: Vec<&ChronicleEvent> = stream
                    .events
                    .iter()
                    .filter(|event| event.op_moment <= op_moment)
                    .collect();
                events.sort_by_key(|event| event.op_moment);

                let mut per_key: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
                for event in events {
                    let Some(key) = record_key(&event.fields, &stream.key) else {
                        continue;
                    };
                    per_key.entry(key).or_default().extend(
                        event
                            .fields
                            .iter()
                            .map(|(name, value)| (name.clone(), value.clone())),
                    );
                }

                ReducedStream {
                    stream: stream.stream.clone(),
                    key: stream.key.clone(),
                    records: per_key.into_values().collect(),
                }
            })
            .collect()
    }
}

/// De sleutelwaarde van een vastlegging, hoofdletterongevoelig opgezocht.
fn record_key(fields: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    fields
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap_or_default()
    }

    fn event(op_moment: &str, fields: &[(&str, Value)]) -> ChronicleEvent {
        ChronicleEvent {
            op_moment: date(op_moment),
            fields: fields
                .iter()
                .map(|(name, value)| ((*name).to_string(), value.clone()))
                .collect(),
        }
    }

    fn store(events: Vec<ChronicleEvent>) -> ChronicleStore {
        ChronicleStore::from_streams(
            "toeslagen",
            vec![ChronicleStream {
                stream: "relatie".to_string(),
                key: "bsn".to_string(),
                events,
            }],
        )
        .unwrap_or_default()
    }

    #[test]
    fn latere_vastlegging_overschrijft_eerdere() {
        let store = store(vec![
            event(
                "2024-01-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("GEEN".to_string())),
                ],
            ),
            event(
                "2024-06-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                ],
            ),
        ]);

        let reduced = store.reduce_to(date("2025-01-01"));
        assert_eq!(reduced.len(), 1);
        assert_eq!(reduced[0].records.len(), 1);
        assert_eq!(
            reduced[0].records[0].get("partnerschap_type"),
            Some(&Value::String("HUWELIJK".to_string()))
        );
    }

    #[test]
    fn feiten_van_na_het_moment_tellen_niet_mee() {
        let store = store(vec![
            event(
                "2024-01-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("GEEN".to_string())),
                ],
            ),
            event(
                "2024-06-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                ],
            ),
        ]);

        let reduced = store.reduce_to(date("2024-03-01"));
        assert_eq!(
            reduced[0].records[0].get("partnerschap_type"),
            Some(&Value::String("GEEN".to_string()))
        );
    }

    #[test]
    fn vastlegging_zonder_sleutel_wordt_geweigerd() {
        let err = ChronicleStore::from_streams(
            "toeslagen",
            vec![ChronicleStream {
                stream: "relatie".to_string(),
                key: "bsn".to_string(),
                events: vec![event("2024-01-01", &[("partnerschap_type", Value::Null)])],
            }],
        )
        .expect_err("een vastlegging zonder sleutelveld hoort te falen");
        assert!(matches!(
            err,
            SimulatorError::ChronicleEventWithoutKey { .. }
        ));
    }

    #[test]
    fn twee_stromen_met_dezelfde_naam_worden_geweigerd() {
        let stream = |events| ChronicleStream {
            stream: "relatie".to_string(),
            key: "bsn".to_string(),
            events,
        };
        let err = ChronicleStore::from_streams(
            "toeslagen",
            vec![
                stream(vec![event(
                    "2024-01-01",
                    &[("bsn", Value::String("1".to_string()))],
                )]),
                stream(vec![event(
                    "2024-02-01",
                    &[("bsn", Value::String("1".to_string()))],
                )]),
            ],
        )
        .expect_err("twee stromen met dezelfde naam horen te falen");
        assert!(
            matches!(err, SimulatorError::DuplicateStream { .. }),
            "verwachtte DuplicateStream, kreeg {err}"
        );
    }

    #[test]
    fn bij_gelijk_moment_wint_de_laatste_uit_de_configuratie() {
        let store = store(vec![
            event(
                "2024-07-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                ],
            ),
            event(
                "2024-07-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("GEEN".to_string())),
                ],
            ),
        ]);

        let reduced = store.reduce_to(date("2025-01-01"));
        assert_eq!(
            reduced[0].records[0].get("partnerschap_type"),
            Some(&Value::String("GEEN".to_string())),
            "de tijdas kan twee vastleggingen op één dag niet ordenen; \
             dan beslist de volgorde in de configuratie"
        );
    }
}
