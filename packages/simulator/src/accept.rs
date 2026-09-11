//! Accepteren: een waarde van een andere cel overnemen in plaats van narekenen.
//!
//! Dit is stap 4 van de RFC-009-beslisboom en invariant I5. Stelt een andere
//! organisatie een feit vast, dan rekent deze cel het niet na: ze vraagt het op,
//! legt vast bij wie ze het ophaalde en op welk moment, en rekent daarmee verder.
//! Het verschil met narekenen is geen nuance — een beschikking van een ander
//! bevoegd gezag opnieuw uitrekenen is het overrulen van dat gezag.
//!
//! Deze module woont **buiten** [`crate::cell`], en dat is de kern van de
//! opzet. Een cel houdt geen veiligheidscontext en geen transport (RFC-022 §2),
//! dus ze kan de grens niet over. Wat ze wel kan, is *zeggen* wat ze nodig heeft
//! ([`crate::AcceptanceRequest`]) en een aangereikte waarde met haar herkomst
//! vastleggen. Het ophalen zelf gebeurt hier, in de laag die de peers kent en
//! de identiteit van de vragende cel draagt: in deze opstelling
//! [`crate::World::decide`].
//!
//! Twee wegen komen hier samen, en ze lopen langs dezelfde context en hetzelfde
//! transport:
//!
//! 1. **`accept_from` in de besluit-definitie** — de cel zegt zelf dat een input
//!    van een andere cel komt. Het besluit vraagt hem op vóór het rekenen.
//! 2. **`source.regulation` in de wet** — het *recht* wijst een andere cel aan
//!    (tier 3 van RFC-022 §4.2). Dan vraagt de engine er tijdens de uitvoering
//!    om, langs de [`CellResolver`] die [`CellBridge`] hier implementeert.
//!
//! Wat er in beide gevallen **niet** gebeurt: de waarde belandt nergens als
//! eigen feit. Niet in een kroniek, niet in het databronregister van de engine.
//! Ze gaat in het decretogram met haar herkomst, en een volgend besluit vraagt
//! opnieuw. Anders zou er een schaduwboekhouding ontstaan die er precies
//! uitziet als eigen wetenschap (RFC-022).

use crate::cell::{AcceptanceRequest, AcceptedSource, Cell, DecretogramInput, InputOrigin};
use crate::error::{Result, SimulatorError};
use crate::security::{Identity, SecurityContext, SignedAnswer};
use crate::transport::{CellTransport, InProcessTransport};
use chrono::NaiveDate;
use regelrecht_engine::{CellResolver, EngineError, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Vraag één waarde op bij een andere cel en maak er een vastlegbare input van.
///
/// Geeft twee dingen terug, en dat is geen gemak: de waarde met haar herkomst
/// voor het decretogram, en het volledige bewijsstuk voor wie het verkeer meet.
/// Het observatielog staat buiten de band (zie [`crate::observation`]), dus het
/// kan het bewijsstuk niet zelf komen halen; wie accepteert, geeft het door.
///
/// Een bron-cel die "niets vastgesteld" antwoordt, laat het besluit omvallen. Dat
/// is geen defect maar een leesbare weigering: welke input van welke cel
/// ontbrak, en wat de bron-cel als reden gaf. Er wordt dan niets vastgelegd — een
/// besluit dat met een gat verder rekent, is erger dan geen besluit.
fn accept_one(
    context: &SecurityContext<'_>,
    cell: &str,
    besluit: &str,
    request: &AcceptanceRequest,
    op_moment: NaiveDate,
) -> Result<(DecretogramInput, SignedAnswer)> {
    let signed = context.query(
        &request.cell,
        &request.lexostatus,
        &request.params,
        op_moment,
    )?;

    let answer = &signed.answer;
    let missing = |reason: String| SimulatorError::AcceptedInputMissing {
        cell: cell.to_string(),
        besluit: besluit.to_string(),
        input: request.input.clone(),
        peer: request.cell.clone(),
        reason,
    };

    let values = answer.values().ok_or_else(|| {
        missing(format!(
            "die stelde voor lexostatus '{}' op {op_moment} niets vast ({})",
            request.lexostatus,
            answer.not_established().unwrap_or_default()
        ))
    })?;

    let value = values.get(&request.field).cloned().ok_or_else(|| {
        missing(format!(
            "haar lexostatus '{}' publiceert geen uitkomst '{}' (wel: {})",
            request.lexostatus,
            request.field,
            values
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    let origin = InputOrigin::Accepted {
        cell: answer.cell.clone(),
        lexostatus: answer.name.clone(),
        field: request.field.clone(),
        op_moment: answer.op_moment,
        asked_by: signed.asked_by.to_string(),
        signature: signed.signature.to_string(),
    };

    Ok((DecretogramInput { value, origin }, signed))
}

/// De cel-tier van de besluit-engine: een [`CellResolver`] die een
/// `source.regulation` naar een andere cel langs dezelfde veiligheidscontext en
/// hetzelfde transport oplost als `accept_from`.
///
/// De brug **bezit** de peers zolang hij bestaat. Dat is geen implementatietruc
/// maar wat de engine vraagt: een geregistreerde resolver moet de uitvoering
/// overleven, dus hij kan de cellen niet lenen. De wereld geeft ze mee voor de
/// duur van één besluit en neemt ze daarna terug ([`Self::release`]). Zolang de
/// brug ze houdt, is de besluitende cel er zelf niet bij — die is uit de map
/// gehaald — en dus is een cel die zichzelf over de grens bevraagt geen
/// afspraak maar een onbereikbaarheid.
///
/// Hij is `pub(crate)`: niemand buiten deze crate hoort een resolver in handen te
/// krijgen, want dat is precies de capability die het reduce-pad níet heeft.
pub(crate) struct CellBridge {
    /// De identiteit van de besluitende cel; zij ondertekent elke vraag.
    identity: Identity,
    /// Het besluit waarvoor gevraagd wordt, voor de foutmeldingen.
    besluit: String,
    /// De afspraken uit de configuratie van de besluitende cel, op
    /// `(cel, uitkomst)` zoals de engine ernaar vraagt.
    sources: BTreeMap<(String, String), AcceptedSource>,
    /// De andere cellen van de wereld, in bezit van de brug tot
    /// [`Self::release`] ze teruggeeft.
    peers: RefCell<BTreeMap<String, Cell>>,
    /// Elk contact dat over de grens ging, in volgorde. Voor het verslag en voor
    /// het observatielog: het log kan niet zelf komen halen wat de engine
    /// onderweg vroeg.
    crossings: RefCell<Vec<SignedAnswer>>,
}

impl CellBridge {
    /// Bouw een brug voor één besluit van één cel.
    pub(crate) fn new(
        identity: Identity,
        besluit: &str,
        sources: impl IntoIterator<Item = AcceptedSource>,
        peers: BTreeMap<String, Cell>,
    ) -> Self {
        Self {
            identity,
            besluit: besluit.to_string(),
            sources: sources
                .into_iter()
                .map(|source| ((source.cell.clone(), source.output.clone()), source))
                .collect(),
            peers: RefCell::new(peers),
            crossings: RefCell::new(Vec::new()),
        }
    }

    /// Vraag één waarde op bij een peer, met een verse context en een vers
    /// transport over de cellen die de brug houdt.
    ///
    /// Het transport wordt per vraag opnieuw gemaakt en niet bewaard: het leent
    /// de peers, en dat is de naad waarlangs later een HTTP-transport aanschuift
    /// zonder dat hier iets verandert.
    fn ask(&self, request: &AcceptanceRequest, op_moment: NaiveDate) -> Result<DecretogramInput> {
        let peers = self.peers.borrow();
        let transport = InProcessTransport::over(&peers);
        let accepted = self.ask_over(&transport, request, op_moment);
        accepted.map(|(input, _)| input)
    }

    /// Dezelfde vraag, met het transport van de aanroeper.
    ///
    /// Apart, zodat het vastleggen van het contact op één plek staat: wat er over
    /// de grens ging, komt in [`Self::crossings`] of het komt nergens, en dan is
    /// het log stil incompleet in plaats van rood.
    fn ask_over(
        &self,
        transport: &dyn CellTransport,
        request: &AcceptanceRequest,
        op_moment: NaiveDate,
    ) -> Result<(DecretogramInput, SignedAnswer)> {
        let context = SecurityContext::new(self.identity.clone(), transport);
        let accepted = accept_one(
            &context,
            self.identity.cell(),
            &self.besluit,
            request,
            op_moment,
        );
        if let Ok((_, signed)) = &accepted {
            self.crossings.borrow_mut().push(signed.clone());
        }
        accepted
    }

    /// Willig elk verzoek van een besluit-definitie in (`accept_from`).
    ///
    /// Dezelfde weg als de tier-3-vragen hierboven, dus hetzelfde log en dezelfde
    /// herkomst. Falen doet het geheel: een besluit met één ontbrekende input
    /// wordt niet genomen.
    pub(crate) fn accept_all(
        &self,
        requests: &[AcceptanceRequest],
        op_moment: NaiveDate,
    ) -> Result<BTreeMap<String, DecretogramInput>> {
        let peers = self.peers.borrow();
        let transport = InProcessTransport::over(&peers);
        let mut accepted = BTreeMap::new();
        for request in requests {
            let (input, _) = self.ask_over(&transport, request, op_moment)?;
            accepted.insert(request.input.clone(), input);
        }
        Ok(accepted)
    }

    /// De contacten die over de grens gingen, in volgorde.
    pub(crate) fn crossings(&self) -> Vec<SignedAnswer> {
        self.crossings.borrow().clone()
    }

    /// Geef de peers terug aan de wereld.
    ///
    /// De brug blijft daarna bestaan — de engine houdt zijn registratie — maar
    /// zonder peers, en een volgend besluit krijgt een verse brug. Dat de engine
    /// een resolver vasthoudt die niets meer kan, is de veilige kant op: een
    /// vraag langs een afgedankte brug loopt dood in plaats van stil een oude
    /// wereld te bevragen.
    pub(crate) fn release(&self) -> BTreeMap<String, Cell> {
        std::mem::take(&mut self.peers.borrow_mut())
    }
}

impl CellResolver for CellBridge {
    fn resolve(
        &self,
        cell_id: &str,
        output: &str,
        parameters: &BTreeMap<String, Value>,
        reference_date: &str,
    ) -> std::result::Result<Option<Value>, EngineError> {
        // De engine laat alleen gedeclareerde cel-ids hier komen, maar niet elke
        // uitkomst van zo'n cel hoeft gedeclareerd te zijn. Een naam zonder
        // afspraak is geen "niets vastgesteld" maar een gat in de configuratie.
        let source = self
            .sources
            .get(&(cell_id.to_string(), output.to_string()))
            .ok_or_else(|| {
                EngineError::ResolutionError(format!(
                    "cel '{}' heeft geen afspraak voor uitkomst '{output}' van cel \
                     '{cell_id}'; vul `accepts_from` aan met de lexostatus die daar \
                     gevraagd moet worden",
                    self.identity.cell()
                ))
            })?;

        let op_moment = NaiveDate::parse_from_str(reference_date, "%Y-%m-%d")
            .map_err(|e| EngineError::InvalidDate(format!("{reference_date}: {e}")))?;

        // De parameters gaan door zoals de engine ze uit `source.parameters`
        // bouwde. De bevraagde cel toetst ze tegen wat zij documenteert, en dat
        // hoort ook: een vraag die niet bij haar gepubliceerde vorm past, mag
        // niet alsnog langs een vertaling hier binnenkomen.
        let request = AcceptanceRequest {
            input: output.to_string(),
            cell: source.cell.clone(),
            lexostatus: source.lexostatus.clone(),
            field: source.field.clone(),
            params: parameters.clone(),
        };

        // De waarde gaat terug naar de engine, die haar in `accepted_values` van
        // het receipt zet en nooit in het databronregister (RFC-013, RFC-022
        // §4.2). Deze brug zet er zelf ook niets neer.
        let accepted = self.ask(&request, op_moment).map_err(|e| match e {
            SimulatorError::Engine(engine) => engine,
            other => EngineError::ResolutionError(other.to_string()),
        })?;
        Ok(Some(accepted.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::CellConfig;
    use std::collections::BTreeSet;
    use std::path::Path;

    /// Een bron-cel met één kroniekfilter; geen corpus nodig, dus geen engine.
    fn brp() -> BTreeMap<String, Cell> {
        let config: CellConfig = serde_yaml_ng::from_str(
            r"
id: brp
laws: []
chronicles:
  - stream: relaties
    key: bsn
    events:
      - name: relatie_gewijzigd
        intake: levering
        recording_actor: brp
        op_moment: 2023-03-01
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
",
        )
        .expect("de testconfiguratie hoort te lezen");
        let cell = Cell::from_config(&config, Path::new("/bestaat-niet"), &BTreeMap::new())
            .expect("een bron-cel heeft geen corpus nodig");
        BTreeMap::from([("brp".to_string(), cell)])
    }

    fn source() -> AcceptedSource {
        AcceptedSource {
            cell: "brp".to_string(),
            output: "partnerschap".to_string(),
            lexostatus: "partnerschap".to_string(),
            field: "partnerschap_type".to_string(),
            doc: None,
        }
    }

    fn bridge() -> CellBridge {
        CellBridge::new(
            Identity::for_cell("toeslagen"),
            "toekenning",
            [source()],
            brp(),
        )
    }

    fn bsn() -> BTreeMap<String, Value> {
        BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))])
    }

    #[test]
    fn een_tier_3_verwijzing_komt_bij_de_lexostatus_van_de_peer_uit() {
        let bridge = bridge();
        let answer = bridge
            .resolve("brp", "partnerschap", &bsn(), "2024-01-01")
            .expect("de afspraak wijst naar een lexostatus die de peer publiceert");

        assert_eq!(answer, Some(Value::String("HUWELIJK".to_string())));
        assert_eq!(
            bridge.crossings().len(),
            1,
            "één vraag hoort één contact te zijn"
        );
        let crossing = &bridge.crossings()[0];
        assert_eq!(crossing.asked_by.cell(), "toeslagen");
        assert_eq!(crossing.answer.cell, "brp");
        assert!(
            crossing.signature.is_simulated(),
            "ook een tier-3-vraag gaat ondertekend over de grens: {}",
            crossing.signature
        );
    }

    #[test]
    fn een_uitkomst_zonder_afspraak_is_een_fout_en_geen_leeg_antwoord() {
        let err = bridge()
            .resolve("brp", "nationaliteit", &bsn(), "2024-01-01")
            .expect_err("een uitkomst die `accepts_from` niet noemt hoort te falen");
        assert!(
            err.to_string().contains("accepts_from"),
            "de melding hoort naar de afspraak te wijzen, kreeg: {err}"
        );
    }

    #[test]
    fn niets_vastgesteld_bij_de_bron_laat_het_besluit_omvallen() {
        // Vóór de vastlegging van de peer: die had toen niets vastgesteld, en dat
        // is haar goed recht. Doorrekenen met een gat is dat niet.
        let err = bridge()
            .resolve("brp", "partnerschap", &bsn(), "2022-01-01")
            .expect_err("zonder feit bij de bron hoort het besluit om te vallen");
        assert!(
            err.to_string().contains("niets vast"),
            "de melding hoort te zeggen dat de bron niets vaststelde, kreeg: {err}"
        );
    }

    #[test]
    fn de_brug_geeft_de_peers_terug() {
        let bridge = bridge();
        let peers = bridge.release();
        assert_eq!(
            peers.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from(["brp"]),
            "de wereld hoort haar cellen ongeschonden terug te krijgen"
        );
        assert!(
            bridge
                .resolve("brp", "partnerschap", &bsn(), "2024-01-01")
                .is_err(),
            "een afgedankte brug hoort dood te lopen en geen oude wereld te bevragen"
        );
    }
}
