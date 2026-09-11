//! Het transport: de enige weg over een celgrens.
//!
//! Dit is de **naad**. Vandaag staan alle cellen in hetzelfde proces, morgen
//! staat er HTTP tussen. Dat verschil mag geen enkele wijziging in
//! [`crate::Cell`] kosten, en daarom is het transport een trait en geen functie.
//!
//! Wie hem aanroept staat vast: uitsluitend de veiligheidscontext van de
//! vragende cel ([`crate::SecurityContext`]). Een cel houdt geen transport — ze
//! heeft geen veld, geen parameter en geen bereikbare weg ernaartoe, en dat is
//! een compileerfout en geen afspraak (RFC-022 §2).
//!
//! ## Één mechanisme, twee soorten peer
//!
//! RFC-022 Open Question 4 vraagt of cel↔cel en cel↔burger twee transports zijn
//! of één mechanisme met twee soorten peer. Deze crate kiest het tweede: er is
//! één trait, en wie er aan de andere kant staat is een eigenschap van de peer
//! en niet van het mechanisme. Dat is een **standpunt in een open vraag**; zie
//! de crate-README.

use crate::cell::{Cell, Lexostatus};
use crate::error::{Result, SimulatorError};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use std::collections::BTreeMap;

/// Eén vraag over een celgrens zetten.
///
/// De vorm is die van de publieke ingang van een cel — een gepubliceerde naam,
/// gedocumenteerde parameters, een moment — en met opzet niets meer. Een
/// transport dat een reductie of een filter zou kunnen meesturen, zou de
/// autonomie van de bevraagde cel omzeilen (RFC-022 §4.1).
///
/// De implementatie beslist hoe de vraag bij de peer komt. Ze beslist niet wie
/// er vraagt en of dat mag: dat is de veiligheidscontext.
pub trait CellTransport {
    /// Vraag `lexostatus` op bij cel `cell`, geldig op `op_moment`.
    ///
    /// Een onbekende peer is een fout. "Op dat moment niets vastgesteld" is dat
    /// niet: dat is een antwoord van de peer en komt als
    /// [`crate::LexostatusOutcome::NotEstablished`] terug.
    fn query(
        &self,
        cell: &str,
        lexostatus: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<Lexostatus>;
}

/// Transport binnen één proces: de peers zijn cellen in dezelfde run.
///
/// Het houdt de cellen **geleend**, niet in bezit. Daarmee is het geen tweede
/// eigenaar van de feiten van een cel: het kan precies één ding met een peer,
/// namelijk hem bevragen langs zijn publieke ingang, net zoals een HTTP-client
/// dat later zal doen.
pub struct InProcessTransport<'world> {
    peers: &'world BTreeMap<String, Cell>,
}

impl<'world> InProcessTransport<'world> {
    /// Maak een transport over de cellen van een run.
    ///
    /// De sleutel van de map is het cel-id waarmee een vrager de peer aanwijst.
    pub fn over(peers: &'world BTreeMap<String, Cell>) -> Self {
        Self { peers }
    }
}

impl CellTransport for InProcessTransport<'_> {
    fn query(
        &self,
        cell: &str,
        lexostatus: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<Lexostatus> {
        let peer = self
            .peers
            .get(cell)
            .ok_or_else(|| SimulatorError::UnknownPeer {
                cell: cell.to_string(),
                known: self
                    .peers
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
            })?;

        // De enige ingang die er is. Het transport kent de kronieken van de peer
        // niet en kan er ook niet bij: `Cell` heeft geen accessor op haar store.
        peer.reduce(lexostatus, params, op_moment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::CellConfig;
    use std::path::Path;

    /// Een bron-cel met één kroniekfilter; geen corpus nodig, dus geen engine.
    fn peer(id: &str) -> Cell {
        let yaml = format!(
            r"
id: {id}
laws: []
chronicles:
  - stream: relaties
    key: bsn
    events:
      - op_moment: 2023-03-01
        fields:
          bsn: '999993653'
          partnerschap_type: HUWELIJK
lexostatus_definitions:
  - name: partnerschap
    inputs:
      - name: bsn
        type: string
    outputs:
      - partnerschap_type
    reduction:
      chronicle: relaties
      key: bsn
      latest: true
"
        );
        let config: CellConfig =
            serde_yaml_ng::from_str(&yaml).expect("de testconfiguratie hoort te lezen");
        Cell::from_config(&config, Path::new("/bestaat-niet"))
            .expect("een bron-cel heeft geen corpus nodig")
    }

    fn world(id: &str) -> BTreeMap<String, Cell> {
        BTreeMap::from([(id.to_string(), peer(id))])
    }

    fn bsn() -> BTreeMap<String, Value> {
        BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))])
    }

    #[test]
    fn een_vraag_aan_een_peer_komt_bij_zijn_publieke_ingang_uit() {
        let cells = world("brp");
        let transport = InProcessTransport::over(&cells);

        let answer = transport
            .query(
                "brp",
                "partnerschap",
                &bsn(),
                NaiveDate::from_ymd_opt(2024, 1, 1).expect("geldige datum"),
            )
            .expect("de peer publiceert deze lexostatus");

        assert_eq!(answer.cell, "brp", "het antwoord zegt wie het gaf");
        assert_eq!(
            answer.values().and_then(|v| v.get("partnerschap_type")),
            Some(&Value::String("HUWELIJK".to_string()))
        );
    }

    #[test]
    fn een_onbekende_peer_is_een_fout_en_geen_leeg_antwoord() {
        let cells = world("brp");
        let transport = InProcessTransport::over(&cells);

        let err = transport
            .query(
                "kadaster",
                "partnerschap",
                &bsn(),
                NaiveDate::from_ymd_opt(2024, 1, 1).expect("geldige datum"),
            )
            .expect_err("een peer die niet bestaat hoort te falen");

        assert!(
            matches!(err, SimulatorError::UnknownPeer { .. }),
            "verwachtte UnknownPeer, kreeg {err}"
        );
    }

    #[test]
    fn het_transport_geeft_een_vraag_ongewijzigd_door() {
        // Een transport dat een lexostatus niet kent, mag er ook niet over
        // beslissen: de peer weigert, niet het transport.
        let cells = world("brp");
        let transport = InProcessTransport::over(&cells);

        let err = transport
            .query(
                "brp",
                "inkomen",
                &bsn(),
                NaiveDate::from_ymd_opt(2024, 1, 1).expect("geldige datum"),
            )
            .expect_err("de peer publiceert deze naam niet");

        assert!(
            matches!(&err, SimulatorError::UnknownLexostatus { cell, .. } if cell == "brp"),
            "de weigering hoort van de peer te komen, kreeg {err}"
        );
    }
}
