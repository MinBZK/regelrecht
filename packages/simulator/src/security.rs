//! De veiligheidscontext: identiteit, ondertekening en transportkeuze.
//!
//! Een van de drie assen van RFC-022 §2, en met opzet een eigen type: de cel
//! houdt geen sleutels en kiest geen transport, en het bevoegd gezag is een
//! juridisch feit van het besluit. Wie die drie op één hoop gooit, krijgt een
//! cel die alles kan en dus niets afdwingt.
//!
//! De context is in deze versie **dun** — een identiteit, een gesimuleerde
//! ondertekening en een geleend transport — maar hij is de enige die
//! [`CellTransport`] aanroept. Dat is het punt: alle verkeer over een celgrens
//! loopt hierlangs, dus er is precies één plek die het kan zien en één plek die
//! het later mag weigeren.

use crate::cell::Lexostatus;
use crate::error::{Result, SimulatorError};
use crate::transport::CellTransport;
use chrono::NaiveDate;
use regelrecht_engine::Value;
use std::collections::BTreeMap;
use std::fmt;

/// Het voorvoegsel van elke gesimuleerde ondertekening.
///
/// Staat letterlijk in de waarde, zodat niemand hem voor echt aanziet en een
/// grep hem vindt zodra er ooit echt ondertekend moet worden. Echte sleutels en
/// trust material zijn buiten scope (RFC-022; Blauwe Knop, FCID en FSC volgen
/// apart).
pub const SIMULATED_SIGNATURE_PREFIX: &str = "GESIMULEERDE-ONDERTEKENING door ";

/// Wie er vraagt.
///
/// De binding van de veiligheidscontext is **Open Question 2** in RFC-022 en
/// dus onbeslist. Deze versie kiest de eenvoudigste vorm die de vraag openhoudt:
/// één identiteit per cel, die de cel zelf is. Er is nog geen medewerker, geen
/// zaak en geen mandaat — komt dat er, dan krijgt dit type velden en hoeft geen
/// enkele aanroeper te veranderen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    cell: String,
}

impl Identity {
    /// De identiteit van een cel.
    pub fn for_cell(cell: &str) -> Self {
        Self {
            cell: cell.to_string(),
        }
    }

    /// De cel waarvoor deze identiteit staat.
    pub fn cell(&self) -> &str {
        &self.cell
    }
}

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cel:{}", self.cell)
    }
}

/// Een **gesimuleerde** ondertekening.
///
/// Niet echt, en dat is zichtbaar in de waarde zelf: er zit geen sleutel achter
/// en er valt niets aan te verifiëren. Wat er wél is, is de vorm — een vraag die
/// de celgrens over gaat draagt de identiteit die hem stuurde — zodat een
/// latere echte ondertekening dezelfde plek inneemt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    value: String,
    signer: Identity,
}

impl Signature {
    /// Onderteken namens deze identiteit.
    fn simulated(signer: &Identity) -> Self {
        Self {
            value: format!("{SIMULATED_SIGNATURE_PREFIX}{signer}"),
            signer: signer.clone(),
        }
    }

    /// De ondertekening zelf, als tekst.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Wie ondertekende.
    pub fn signer(&self) -> &Identity {
        &self.signer
    }

    /// Is dit een placeholder in plaats van een echte ondertekening? Altijd
    /// waar, zolang er geen trust material is; een test die dit controleert
    /// wordt rood op de dag dat dat verandert.
    pub fn is_simulated(&self) -> bool {
        self.value.starts_with(SIMULATED_SIGNATURE_PREFIX)
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

/// Het bewijsstuk van één vraag over een celgrens: wat er gevraagd is, door
/// wie, ondertekend, en wat er terugkwam.
///
/// De context geeft dit terug in plaats van alleen het antwoord, en dat is geen
/// gemak. Een cel die straks een waarde van een andere cel *accepteert*, moet in
/// haar decretogram kunnen vastleggen bij wie ze het ophaalde, onder welke naam,
/// op welk moment en ondertekend door wie (RFC-013 `accepted_values`). Datzelfde
/// bewijsstuk is wat het observatielog nodig heeft.
#[derive(Debug, Clone)]
pub struct SignedAnswer {
    /// De identiteit die de vraag stelde en ondertekende.
    pub asked_by: Identity,
    /// De gesimuleerde ondertekening van de vraag.
    pub signature: Signature,
    /// De parameters waarmee gevraagd is.
    pub params: BTreeMap<String, Value>,
    /// Het antwoord van de peer. Draagt zelf de bevraagde cel, de gevraagde
    /// naam en het moment waarop het geldt.
    pub answer: Lexostatus,
}

/// De veiligheidscontext van één cel.
///
/// Gebonden aan precies één cel: de identiteit die hij houdt, is die van die
/// cel. Hij leent een transport en is de enige die het aanroept.
pub struct SecurityContext<'a> {
    identity: Identity,
    transport: &'a dyn CellTransport,
}

impl<'a> SecurityContext<'a> {
    /// Bind een context aan een identiteit en een transport.
    pub fn new(identity: Identity, transport: &'a dyn CellTransport) -> Self {
        Self {
            identity,
            transport,
        }
    }

    /// De identiteit waaronder deze context vraagt.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// De cel waaraan deze context gebonden is.
    pub fn cell(&self) -> &str {
        self.identity.cell()
    }

    /// Vraag een lexostatus op bij een andere cel.
    ///
    /// De context ondertekent de vraag (gesimuleerd) en zet hem over de grens.
    /// Hij combineert niets en onthoudt niets: het bewijsstuk gaat terug naar de
    /// aanroeper, die het in zijn besluit vastlegt. Synthese hoort bij een
    /// consument, nooit in een cel (RFC-022 §4.1).
    ///
    /// Een vraag aan de eigen cel wordt geweigerd. Daarvoor is er geen transport
    /// nodig maar een reductie, en het verschil is precies wat deze opstelling
    /// meet: een cel die zichzelf via de grens bevraagt, zou als cross-cel-
    /// contact in het log komen en het vraaggraf vervuilen.
    pub fn query(
        &self,
        cell: &str,
        lexostatus: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<SignedAnswer> {
        if cell == self.identity.cell() {
            return Err(SimulatorError::TransportToSelf {
                cell: cell.to_string(),
                lexostatus: lexostatus.to_string(),
            });
        }

        let signature = Signature::simulated(&self.identity);
        let answer = self.transport.query(cell, lexostatus, params, op_moment)?;

        Ok(SignedAnswer {
            asked_by: self.identity.clone(),
            signature,
            params: params.clone(),
            answer,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::LexostatusOutcome;
    use std::cell::RefCell;

    /// Een transport dat niets doet behalve opschrijven dat het geroepen werd.
    /// Zo is te zien dat de context wél of juist niet over de grens gaat.
    struct SpyTransport {
        calls: RefCell<Vec<String>>,
    }

    impl SpyTransport {
        fn new() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CellTransport for SpyTransport {
        fn query(
            &self,
            cell: &str,
            lexostatus: &str,
            _params: &BTreeMap<String, Value>,
            op_moment: NaiveDate,
        ) -> Result<Lexostatus> {
            self.calls.borrow_mut().push(format!("{cell}.{lexostatus}"));
            Ok(Lexostatus {
                cell: cell.to_string(),
                name: lexostatus.to_string(),
                op_moment,
                outcome: LexostatusOutcome::Established(BTreeMap::new()),
            })
        }
    }

    fn moment() -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 1, 1).expect("geldige datum")
    }

    #[test]
    fn de_context_ondertekent_elke_vraag_gesimuleerd() {
        let transport = SpyTransport::new();
        let context = SecurityContext::new(Identity::for_cell("toeslagen"), &transport);

        let signed = context
            .query("brp", "partnerschap", &BTreeMap::new(), moment())
            .expect("de vraag hoort door te gaan");

        assert_eq!(signed.asked_by, Identity::for_cell("toeslagen"));
        assert!(
            signed.signature.is_simulated(),
            "de ondertekening hoort herkenbaar nep te zijn: {}",
            signed.signature
        );
        assert!(
            signed.signature.value().contains("cel:toeslagen"),
            "de ondertekening hoort de identiteit te noemen: {}",
            signed.signature
        );
    }

    #[test]
    fn een_vraag_aan_de_eigen_cel_gaat_niet_over_de_grens() {
        let transport = SpyTransport::new();
        let context = SecurityContext::new(Identity::for_cell("toeslagen"), &transport);

        let err = context
            .query("toeslagen", "partnerschap", &BTreeMap::new(), moment())
            .expect_err("de eigen cel is geen peer");

        assert!(
            matches!(err, SimulatorError::TransportToSelf { .. }),
            "verwachtte TransportToSelf, kreeg {err}"
        );
        assert!(
            transport.calls.borrow().is_empty(),
            "er had geen transport aan te pas mogen komen"
        );
    }

    #[test]
    fn het_bewijsstuk_draagt_de_vraag_zoals_die_gesteld_is() {
        let transport = SpyTransport::new();
        let context = SecurityContext::new(Identity::for_cell("toeslagen"), &transport);
        let params = BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))]);

        let signed = context
            .query("brp", "partnerschap", &params, moment())
            .expect("de vraag hoort door te gaan");

        assert_eq!(signed.params, params, "de parameters horen erbij te staan");
        assert_eq!(signed.answer.cell, "brp");
        assert_eq!(signed.answer.name, "partnerschap");
        assert_eq!(signed.answer.op_moment, moment());
        assert_eq!(
            transport.calls.borrow().as_slice(),
            ["brp.partnerschap"],
            "precies één keer over de grens"
        );
    }
}
