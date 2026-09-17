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

use crate::cell::{
    AcceptanceRequest, AcceptedSource, Cell, DecretogramInput, InputOrigin, COMPETENT_AUTHORITY,
};
use crate::error::{Result, SimulatorError};
use crate::security::{Identity, SecurityContext, SignedAnswer};
use crate::transport::{CellTransport, InProcessTransport};
use chrono::NaiveDate;
use regelrecht_engine::{CellResolver, EngineError, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Maak van een bewijsstuk de vastlegbare input die het besluit nodig heeft.
///
/// Losgekoppeld van het stellen van de vraag, en dat is het hele punt van de
/// splitsing: het contact over de grens is al een feit als deze functie begint.
/// Wat hier nog kan mislukken, is het *lezen* van het antwoord — en dat mag het
/// bewijs van de vraag niet meer wissen.
///
/// Een bron-cel die "niets vastgesteld" antwoordt, laat het besluit omvallen. Dat
/// is geen defect maar een leesbare weigering: welke input van welke cel
/// ontbrak, en wat de bron-cel als reden gaf. Er wordt dan niets vastgelegd — een
/// besluit dat met een gat verder rekent, is erger dan geen besluit.
///
/// `contact` is het volgnummer van dat bewijsstuk onder de contacten van dit
/// besluit (vanaf 1). Het komt als verwijzing in de herkomst: lezen meerdere
/// inputs uit één antwoord, dan staat bij elk van hen hetzelfde nummer.
fn accepted_input(
    cell: &str,
    besluit: &str,
    request: &AcceptanceRequest,
    signed: &SignedAnswer,
    contact: usize,
) -> Result<DecretogramInput> {
    let answer = &signed.answer;
    let op_moment = answer.op_moment;
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
        // Het gezag komt uit het antwoord zelf en niet uit een lijst hier: wie
        // bevoegd is om dit vast te stellen, weet de bron-cel en niemand anders.
        // Publiceert haar lexostatus er niets over, dan staat er `None` — een gat
        // bij de bron dat een lezer hoort te zien.
        authority: values
            .get(COMPETENT_AUTHORITY)
            .and_then(Value::as_str)
            .map(str::to_string),
        lexostatus: answer.name.clone(),
        field: request.field.clone(),
        op_moment: answer.op_moment,
        asked_by: signed.asked_by.to_string(),
        signature: signed.signature.to_string(),
        contact,
    };

    Ok(DecretogramInput { value, origin })
}

/// De verzoeken van één besluit, gebundeld per vraag die over de grens gaat.
///
/// Twee verzoeken stellen dezelfde vraag als ze bij dezelfde cel dezelfde
/// lexostatus met dezelfde parameters opvragen; het veld doet er niet toe. De
/// volgorde is die van het eerste verzoek per vraag, zodat de contacten in de
/// volgorde van de definitie blijven staan. Nooit leeg per bundel.
fn same_questions(requests: &[AcceptanceRequest]) -> Vec<Vec<&AcceptanceRequest>> {
    let mut groups: Vec<Vec<&AcceptanceRequest>> = Vec::new();
    for request in requests {
        let same = groups.iter_mut().find(|group| {
            let first = group[0];
            first.cell == request.cell
                && first.lexostatus == request.lexostatus
                && first.params == request.params
        });
        match same {
            Some(group) => group.push(request),
            None => groups.push(vec![request]),
        }
    }
    groups
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
        let (signed, contact) = self.cross(&transport, request, op_moment)?;
        accepted_input(
            self.identity.cell(),
            &self.besluit,
            request,
            &signed,
            contact,
        )
    }

    /// Zet één vraag over de grens, met het transport van de aanroeper, en geef
    /// het bewijsstuk terug met zijn volgnummer onder de contacten van dit
    /// besluit.
    ///
    /// Apart, zodat het vastleggen van het contact op één plek staat: wat er over
    /// de grens ging, komt in [`Self::crossings`] of het komt nergens, en dan is
    /// het log stil incompleet in plaats van rood.
    ///
    /// Het contact wordt vastgelegd zodra de peer antwoordde, en niet pas als dat
    /// antwoord bruikbaar blijkt. Een "niets vastgesteld" is óók over de grens
    /// gegaan: de vraag is gesteld, ondertekend, en de peer heeft haar gezien.
    /// Zou het bewijsstuk pas bij een bruikbare waarde opgeschreven worden, dan
    /// zou precies het geval dat het besluit doet omvallen in het vraaggraf
    /// onzichtbaar blijven — en dat is de gevaarlijke kant op.
    ///
    /// Alleen de vraag telt hier — cel, lexostatus, parameters, moment — en niet
    /// het veld: welke uitkomst eruit gelezen wordt, is iets van de vrager en
    /// gaat niet over de grens.
    fn cross(
        &self,
        transport: &dyn CellTransport,
        question: &AcceptanceRequest,
        op_moment: NaiveDate,
    ) -> Result<(SignedAnswer, usize)> {
        let context = SecurityContext::new(self.identity.clone(), transport);
        let signed = context.query(
            &question.cell,
            &question.lexostatus,
            &question.params,
            op_moment,
        )?;
        let mut crossings = self.crossings.borrow_mut();
        crossings.push(signed.clone());
        Ok((signed, crossings.len()))
    }

    /// Willig elk verzoek van een besluit-definitie in (`accept_from`).
    ///
    /// Dezelfde weg als de tier-3-vragen hierboven, dus hetzelfde log en dezelfde
    /// herkomst. Falen doet het geheel: een besluit met één ontbrekende input
    /// wordt niet genomen.
    ///
    /// **Eén vraag per antwoord, niet per input.** Lezen meerdere inputs uit
    /// dezelfde lexostatus van dezelfde cel, met dezelfde parameters — het moment
    /// is voor één besluit altijd hetzelfde — dan gaat die vraag één keer over de
    /// grens, en lezen ze alle hun eigen veld uit dat ene antwoord. Elke vraag is
    /// bij de bron een gelogde verwerking; dezelfde vraag drie keer stellen
    /// vertelt de bron niets nieuws en het vraaggraf ook niet. Wat per input
    /// blijft, is de herkomst in het gram: elk veld noemt zijn eigen cel,
    /// lexostatus, veld en moment, en verwijst met hetzelfde contactnummer naar
    /// het gedeelde antwoord.
    ///
    /// Groeperen gebeurt binnen dit ene besluit en nergens daarbuiten: een
    /// volgend besluit krijgt een verse brug en vraagt opnieuw bij de bron.
    /// Onthouden tussen besluiten zou een schaduwboekhouding zijn.
    pub(crate) fn accept_all(
        &self,
        requests: &[AcceptanceRequest],
        op_moment: NaiveDate,
    ) -> Result<BTreeMap<String, DecretogramInput>> {
        let peers = self.peers.borrow();
        let transport = InProcessTransport::over(&peers);
        let mut accepted = BTreeMap::new();
        for group in same_questions(requests) {
            let (signed, contact) = self.cross(&transport, group[0], op_moment)?;
            for request in group {
                let input = accepted_input(
                    self.identity.cell(),
                    &self.besluit,
                    request,
                    &signed,
                    contact,
                )?;
                accepted.insert(request.input.clone(), input);
            }
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
          partner_bsn: '999990019'
          ingangsdatum: '2023-03-01'
      - name: relatie_gewijzigd
        intake: levering
        recording_actor: brp
        op_moment: 2023-04-01
        fields:
          bsn: '999990019'
          partnerschap_type: HUWELIJK
          partner_bsn: '999993653'
          ingangsdatum: '2023-03-01'
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
  - name: relatie
    inputs:
      - name: bsn
        type: string
    outputs:
      - partnerschap_type
      - partner_bsn
      - ingangsdatum
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
        let bridge = bridge();
        let err = bridge
            .resolve("brp", "partnerschap", &bsn(), "2022-01-01")
            .expect_err("zonder feit bij de bron hoort het besluit om te vallen");
        assert!(
            err.to_string().contains("niets vast"),
            "de melding hoort te zeggen dat de bron niets vaststelde, kreeg: {err}"
        );

        // En de vraag stáát er, want ze is gesteld. Dat het antwoord het besluit
        // niet verder helpt, maakt het contact niet ongedaan: wie het vraaggraf
        // meet, hoort juist dit geval te zien.
        assert_eq!(
            bridge.crossings().len(),
            1,
            "een 'niets vastgesteld' is óók over de grens gegaan"
        );
        let crossing = &bridge.crossings()[0];
        assert_eq!(crossing.answer.cell, "brp");
        assert!(
            crossing.answer.not_established().is_some(),
            "en het bewijsstuk hoort te dragen wat de peer antwoordde"
        );
    }

    /// Een `accept_from`-verzoek zoals een besluit-definitie het opstelt.
    fn request(input: &str, lexostatus: &str, field: &str, bsn: &str) -> AcceptanceRequest {
        AcceptanceRequest {
            input: input.to_string(),
            cell: "brp".to_string(),
            lexostatus: lexostatus.to_string(),
            field: field.to_string(),
            params: BTreeMap::from([("bsn".to_string(), Value::String(bsn.to_string()))]),
        }
    }

    fn moment() -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 1, 1).expect("geldige datum")
    }

    /// Het contactnummer en het veld van een geaccepteerde input.
    fn contact_of(input: &DecretogramInput) -> (usize, &str) {
        match &input.origin {
            InputOrigin::Accepted { contact, field, .. } => (*contact, field.as_str()),
            other => panic!("verwachtte een geaccepteerde herkomst, kreeg {other:?}"),
        }
    }

    #[test]
    fn drie_inputs_uit_een_lexostatus_zijn_een_vraag() {
        let bridge = bridge();
        let accepted = bridge
            .accept_all(
                &[
                    request("type", "relatie", "partnerschap_type", "999993653"),
                    request("partner", "relatie", "partner_bsn", "999993653"),
                    request("sinds", "relatie", "ingangsdatum", "999993653"),
                ],
                moment(),
            )
            .expect("de lexostatus publiceert alle drie de velden");

        assert_eq!(
            bridge.crossings().len(),
            1,
            "dezelfde vraag hoort één keer over de grens te gaan"
        );
        // Elk veld leest uit dat ene antwoord, en houdt zijn eigen herkomst.
        assert_eq!(contact_of(&accepted["type"]), (1, "partnerschap_type"));
        assert_eq!(contact_of(&accepted["partner"]), (1, "partner_bsn"));
        assert_eq!(contact_of(&accepted["sinds"]), (1, "ingangsdatum"));
        assert_eq!(
            accepted["partner"].value,
            Value::String("999990019".to_string())
        );
        assert_eq!(
            accepted["type"].value,
            Value::String("HUWELIJK".to_string())
        );
    }

    #[test]
    fn twee_lexostatussen_zijn_twee_vragen() {
        let bridge = bridge();
        let accepted = bridge
            .accept_all(
                &[
                    request("type", "partnerschap", "partnerschap_type", "999993653"),
                    request("partner", "relatie", "partner_bsn", "999993653"),
                    request("sinds", "relatie", "ingangsdatum", "999993653"),
                ],
                moment(),
            )
            .expect("beide lexostatussen publiceren wat gevraagd wordt");

        let names: Vec<String> = bridge
            .crossings()
            .iter()
            .map(|signed| signed.answer.name.clone())
            .collect();
        assert_eq!(
            names,
            ["partnerschap", "relatie"],
            "één vraag per lexostatus"
        );
        assert_eq!(contact_of(&accepted["type"]).0, 1);
        assert_eq!(contact_of(&accepted["partner"]).0, 2);
        assert_eq!(contact_of(&accepted["sinds"]).0, 2);
    }

    #[test]
    fn andere_parameters_zijn_een_aparte_vraag() {
        let bridge = bridge();
        let accepted = bridge
            .accept_all(
                &[
                    request("eigen", "relatie", "partner_bsn", "999993653"),
                    request("partner", "relatie", "partner_bsn", "999990019"),
                ],
                moment(),
            )
            .expect("beide personen staan bij de bron");

        assert_eq!(
            bridge.crossings().len(),
            2,
            "een vraag over een andere persoon is een andere vraag"
        );
        assert_eq!(contact_of(&accepted["eigen"]).0, 1);
        assert_eq!(contact_of(&accepted["partner"]).0, 2);
        assert_eq!(
            accepted["partner"].value,
            Value::String("999993653".to_string())
        );
    }

    #[test]
    fn een_volgend_besluit_vraagt_opnieuw() {
        // Groeperen gaat niet over besluiten heen: een verse brug weet niets van
        // wat een vorige vroeg.
        let requests = [request("type", "relatie", "partnerschap_type", "999993653")];
        let eerste = bridge();
        eerste
            .accept_all(&requests, moment())
            .expect("de vraag hoort te slagen");
        let tweede = CellBridge::new(
            Identity::for_cell("toeslagen"),
            "toekenning",
            [source()],
            eerste.release(),
        );
        tweede
            .accept_all(&requests, moment())
            .expect("de vraag hoort te slagen");
        assert_eq!(eerste.crossings().len(), 1);
        assert_eq!(
            tweede.crossings().len(),
            1,
            "het tweede besluit vraagt zelf"
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
