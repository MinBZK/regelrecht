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
use crate::values::equivalent;
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

    /// De veldnamen die deze cel van elke stroom kent.
    ///
    /// Het sleutelveld hoort er altijd bij — de stroom declareert het — en
    /// verder alles wat in een vastlegging voorkomt. Hiermee valt bij het
    /// optuigen te toetsen of een kroniekfilter praat over velden die bestaan,
    /// in plaats van stil "niets vastgesteld" te antwoorden op een typfout.
    pub(crate) fn declared_fields(&self) -> BTreeMap<String, BTreeSet<String>> {
        self.streams
            .iter()
            .map(|stream| {
                let mut fields: BTreeSet<String> = BTreeSet::from([stream.key.clone()]);
                for event in &stream.events {
                    fields.extend(event.fields.keys().cloned());
                }
                (stream.stream.clone(), fields)
            })
            .collect()
    }

    /// De laatste vastlegging op of vóór `op_moment` met deze sleutelwaarde.
    ///
    /// Dit is de reductie van een cel zonder engine: geen toestandsmerge over
    /// velden heen, maar precies één vastlegging. Wat samen vastgelegd is,
    /// blijft samen — de eigenschap die [`Self::reduce_to`] opgeeft omdat de
    /// engine records als databron wil.
    ///
    /// `conditions` bepaalt wélke vastleggingen meedoen, en pas daarna wint de
    /// laatste. Een voorwaarde op een veld dat over tijd verandert levert dus de
    /// laatste vastlegging die eraan voldeed, niet de huidige stand.
    ///
    /// `None` is een antwoord en geen fout: op dit moment was er niets
    /// vastgesteld over dit onderwerp.
    pub(crate) fn latest_recording(
        &self,
        stream: &str,
        key: &str,
        key_value: &Value,
        conditions: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Option<&ChronicleEvent> {
        // `max_by_key` levert bij gelijke sleutel het laatste element, dus twee
        // vastleggingen op één dag volgen dezelfde regel als in `reduce_to`: de
        // volgorde in de configuratie beslist.
        self.streams
            .iter()
            .find(|candidate| candidate.stream == stream)?
            .events
            .iter()
            .filter(|event| event.op_moment <= op_moment)
            .filter(|event| field_equals(&event.fields, key, key_value))
            .filter(|event| {
                conditions
                    .iter()
                    .all(|(field, expected)| field_equals(&event.fields, field, expected))
            })
            .max_by_key(|event| event.op_moment)
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

/// De waarde van een veld, hoofdletterongevoelig opgezocht.
///
/// Zelfde souplesse als de engine, die inputnamen ook hoofdletterongevoelig
/// matcht: een vastlegging die `BSN` schrijft, gaat over hetzelfde veld als een
/// die `bsn` schrijft.
pub(crate) fn field<'a>(fields: &'a BTreeMap<String, Value>, name: &str) -> Option<&'a Value> {
    fields
        .iter()
        .find(|(field, _)| field.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

/// Heeft deze vastlegging dit veld, met deze waarde?
///
/// Een veld dat de vastlegging niet heeft, voldoet niet: "onbekend" is geen
/// gelijkheid.
fn field_equals(fields: &BTreeMap<String, Value>, name: &str, expected: &Value) -> bool {
    field(fields, name).is_some_and(|value| equivalent(value, expected))
}

/// De sleutelwaarde van een vastlegging, hoofdletterongevoelig opgezocht.
fn record_key(fields: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    field(fields, key).map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faalt luid op een onleesbare datum: deze tests gáán over de tijdas, dus
    /// een typfout die stil op de standaarddatum uitkomt zou een gebroken test
    /// achter een onschuldige assertie verstoppen.
    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
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
        .unwrap_or_else(|e| panic!("teststore moet op te bouwen zijn: {e}"))
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

    #[test]
    fn het_filter_volgt_bij_gelijk_moment_dezelfde_regel_als_de_tijdreductie() {
        // `latest_recording` leunt hiervoor op de belofte van `max_by_key` dat
        // bij gelijke sleutel het laatste element wint. Zonder deze test zou een
        // andere formulering (`max_by`, eerst sorteren, omgekeerd doorlopen) het
        // antwoord van een bron-cel stil omdraaien, terwijl de tijdreductie
        // ernaast wél bewaakt blijft.
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

        let found = store
            .latest_recording(
                "relatie",
                "bsn",
                &Value::String("1".to_string()),
                &BTreeMap::new(),
                date("2025-01-01"),
            )
            .unwrap_or_else(|| panic!("er staan twee vastleggingen, dus er is er een de laatste"));
        assert_eq!(
            found.fields.get("partnerschap_type"),
            Some(&Value::String("GEEN".to_string())),
            "het kroniekfilter moet dezelfde vastlegging kiezen als de tijdreductie"
        );
    }

    #[test]
    fn een_voorwaarde_op_een_veld_dat_de_vastlegging_niet_heeft_voldoet_niet() {
        // "Onbekend" is geen gelijkheid: een vastlegging die het veld niet draagt
        // doet niet mee, ook niet als ze op de tijdas de laatste zou zijn.
        let store = store(vec![
            event(
                "2023-03-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                ],
            ),
            event("2024-07-01", &[("bsn", Value::String("1".to_string()))]),
        ]);

        let found = store
            .latest_recording(
                "relatie",
                "bsn",
                &Value::String("1".to_string()),
                &BTreeMap::from([(
                    "partnerschap_type".to_string(),
                    Value::String("HUWELIJK".to_string()),
                )]),
                date("2025-01-01"),
            )
            .unwrap_or_else(|| panic!("de vastlegging van 2023-03-01 voldoet aan het filter"));
        assert_eq!(
            found.op_moment,
            date("2023-03-01"),
            "de latere vastlegging kent het veld niet en doet dus niet mee"
        );
    }
}
