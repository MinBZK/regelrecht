//! De cel: een containment- en autonomiedomein, en verder niets.
//!
//! Een cel houdt kronieken, reduceert daarover en biedt het resultaat aan als
//! lexostatus. Ze houdt geen sleutels, geen bevoegdheid en geen transport —
//! dat zijn eigen assen (RFC-022 §2).
//!
//! De reductielogica woont hier, niet in de scenario-runner. De runner leest
//! configuratie en stelt vragen; wat een reductie inhoudt, weet alleen de cel.

mod chronicle;
mod config;

pub use chronicle::{ChronicleEvent, ChronicleStore, ChronicleStream};
pub use config::{CellConfig, LexostatusDefinition, LexostatusInput, ParameterType, Reduction};

use crate::corpus;
use crate::error::{Result, SimulatorError};
use chrono::NaiveDate;
use config::{engine_parameters, CellSurface};
use regelrecht_engine::{LawExecutionService, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

/// Het antwoord van een cel: de rechtstoestand vanuit een gevraagd perspectief,
/// op de feiten die in die cel bekend zijn.
#[derive(Debug, Clone)]
pub struct Lexostatus {
    /// De cel die geantwoord heeft.
    pub cell: String,
    /// De gepubliceerde naam die gevraagd werd.
    pub name: String,
    /// Het moment waarop gevraagd is; het antwoord geldt op dat moment.
    pub op_moment: NaiveDate,
    /// Wat de cel op dat moment vond.
    pub outcome: LexostatusOutcome,
}

/// De twee antwoorden die een reductie kan opleveren.
///
/// "Niets vastgesteld" is er één van. Een cel die op het gevraagde moment geen
/// feit had, heeft niet gefaald en is niet stuk; ze heeft een antwoord dat een
/// consument moet kunnen onderscheiden van een antwoord met waarden. Een lege
/// map zou dat onderscheid verstoppen, want die lijkt op een antwoord.
#[derive(Debug, Clone)]
pub enum LexostatusOutcome {
    /// Er was een feit, en dit is wat de cel erover publiceert.
    ///
    /// Uitsluitend de uitkomsten die de definitie publiceert. De engine levert
    /// bij een gevraagde uitkomst ook wat er causaal mee meekomt; berekend is
    /// niet gepubliceerd, dus dat blijft binnen de cel.
    Established(BTreeMap<String, Value>),
    /// Er was op het gevraagde moment niets vastgesteld.
    NotEstablished {
        /// Waarom er niets was, in de woorden van de cel: welke stroom is
        /// nagekeken, met welke sleutel en welk filter.
        reason: String,
    },
}

impl Lexostatus {
    /// De gepubliceerde waarden, of `None` als er niets vastgesteld was.
    pub fn values(&self) -> Option<&BTreeMap<String, Value>> {
        match &self.outcome {
            LexostatusOutcome::Established(values) => Some(values),
            LexostatusOutcome::NotEstablished { .. } => None,
        }
    }

    /// Waarom er niets vastgesteld was, of `None` als er wél een feit was.
    pub fn not_established(&self) -> Option<&str> {
        match &self.outcome {
            LexostatusOutcome::Established(_) => None,
            LexostatusOutcome::NotEstablished { reason } => Some(reason),
        }
    }
}

/// Eén chronolexocel.
///
/// De cel bezit haar feiten. Er is met opzet geen `pub fn store()` en geen
/// publiek veld: dat een andere cel niet bij deze kronieken kan, is een
/// compileerfout en geen afspraak. De enige publieke ingang voor een consument
/// is [`Cell::reduce`].
pub struct Cell {
    /// Het cel-id, alleen voor foutmeldingen en herkomst in het antwoord.
    id: String,
    /// Engine met uitsluitend de eigen wetten van deze cel geladen — of geen
    /// engine, bij een bron-cel (`laws: []`).
    ///
    /// Dat dit een `Option` is, is de vorm van RFC-022 §2: de engine is een
    /// component dat in een cel kán draaien, niet de cel zelf. Een organisatie
    /// die niet op RegelRecht draait, legt vast en reduceert, en is daarmee een
    /// gewone cel.
    ///
    /// In een `RefCell`, omdat elke reductie de zichtbare feiten opnieuw op het
    /// gevraagde moment zet; naar buiten toe blijft bevragen een leesactie.
    service: Option<RefCell<LawExecutionService>>,
    /// De eigen feiten. Privé, en dat is het punt.
    chronicles: ChronicleStore,
    /// De gepubliceerde lexostatussen, op naam.
    published: BTreeMap<String, LexostatusDefinition>,
}

impl Cell {
    /// Tuig een cel op uit haar configuratie.
    ///
    /// Laadt uitsluitend de eigen wetten (alle versies, zodat de engine zelf op
    /// het gevraagde moment de juiste kiest) en controleert de gepubliceerde
    /// lexostatussen voordat er ook maar één vraag gesteld kan worden.
    ///
    /// `laws: []` is geldig: dan komt er geen engine, en houdt de cel het bij
    /// vastleggen en reduceren over haar eigen kronieken.
    pub fn from_config(config: &CellConfig, regulation_root: &Path) -> Result<Self> {
        let chronicles = ChronicleStore::from_streams(&config.id, config.chronicles.clone())?;

        let service = if config.laws.is_empty() {
            None
        } else {
            let mut service = LawExecutionService::new();
            for law in &config.laws {
                for document in corpus::regulation_versions(regulation_root, law)? {
                    service.load_law(&document)?;
                }
            }
            Some(service)
        };

        let surface = CellSurface {
            laws: &config.laws,
            outputs: service
                .as_ref()
                .map(outputs_per_regulation)
                .unwrap_or_default(),
            streams: chronicles.declared_fields(),
        };

        let mut published: BTreeMap<String, LexostatusDefinition> = BTreeMap::new();
        for definition in &config.lexostatus_definitions {
            definition.validate(&config.id, &surface)?;
            if published
                .insert(definition.name.clone(), definition.clone())
                .is_some()
            {
                return Err(SimulatorError::DuplicateLexostatus {
                    cell: config.id.clone(),
                    name: definition.name.clone(),
                });
            }
        }

        Ok(Self {
            id: config.id.clone(),
            service: service.map(RefCell::new),
            chronicles,
            published,
        })
    }

    /// Reduceer over de eigen feiten en lever de gevraagde lexostatus.
    ///
    /// Dit is de enige ingang die een consument heeft. Hij kiest een
    /// gepubliceerde naam en levert de gedocumenteerde parameters; hoe er
    /// gereduceerd wordt, bepaalt de cel. Een onbekende naam is een nette fout
    /// die opsomt wat de cel wél publiceert.
    ///
    /// Het antwoord draagt uitsluitend de uitkomsten die de definitie
    /// publiceert. Gedocumenteerd geldt dus aan beide kanten: de vraag mag
    /// alleen wat de definitie noemt, en het antwoord geeft niet meer dan dat.
    ///
    /// `op_moment` is het moment waarop gevraagd wordt: feiten die pas later in
    /// deze cel zijn vastgelegd, bestaan voor dit antwoord niet. Was er op dat
    /// moment niets vastgesteld, dan is dat een antwoord — zie
    /// [`LexostatusOutcome`] — en geen fout.
    pub fn reduce(
        &self,
        lexostatus: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<Lexostatus> {
        let definition =
            self.published
                .get(lexostatus)
                .ok_or_else(|| SimulatorError::UnknownLexostatus {
                    cell: self.id.clone(),
                    requested: lexostatus.to_string(),
                    published: self
                        .published
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                })?;

        definition.check_params(&self.id, params)?;

        let outcome = match &definition.reduction {
            Reduction::Law {
                regulation,
                output,
                parameters,
            } => self.reduce_with_law(
                definition, regulation, output, parameters, params, op_moment,
            )?,
            Reduction::Chronicle {
                chronicle,
                key,
                conditions,
            } => {
                self.filter_chronicle(definition, chronicle, key, conditions, params, op_moment)?
            }
        };

        Ok(Lexostatus {
            cell: self.id.clone(),
            name: definition.name.clone(),
            op_moment,
            outcome,
        })
    }

    /// De wetsvorm: laat de eigen engine over de eigen feiten rekenen.
    fn reduce_with_law(
        &self,
        definition: &LexostatusDefinition,
        regulation: &str,
        output: &str,
        parameters: &BTreeMap<String, String>,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<LexostatusOutcome> {
        // Onbereikbaar: `validate` weigert bij het optuigen elke wetsvorm over
        // een regeling die de cel niet zelf laadt, en een cel zonder engine
        // laadt er geen enkele. De melding is hier dan ook de juiste.
        let Some(service) = &self.service else {
            return Err(SimulatorError::ForeignRegulation {
                cell: self.id.clone(),
                lexostatus: definition.name.clone(),
                regulation: regulation.to_string(),
            });
        };

        let mut service = service.borrow_mut();
        service.clear_data_sources();
        for stream in self.chronicles.reduce_to(op_moment) {
            service.register_dict_source(&stream.stream, &stream.key, stream.records)?;
        }

        let result = service.evaluate_law_output(
            regulation,
            output,
            engine_parameters(parameters, params),
            &op_moment.format("%Y-%m-%d").to_string(),
        )?;

        Ok(LexostatusOutcome::Established(
            definition.project(result.outputs),
        ))
    }

    /// Het kroniekfilter: lees de laatste vastlegging over dit onderwerp.
    ///
    /// Geen engine in zicht. Wat de vastlegging draagt en de definitie
    /// publiceert, komt in het antwoord; een gepubliceerd veld dat deze
    /// vastlegging niet heeft, blijft eruit — de cel vult niets aan.
    fn filter_chronicle(
        &self,
        definition: &LexostatusDefinition,
        chronicle: &str,
        key: &str,
        conditions: &BTreeMap<String, Value>,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<LexostatusOutcome> {
        // Onbereikbaar: `validate` eist dat de sleutel een gedocumenteerde
        // parameter is, en `check_params` dat elke gedocumenteerde parameter
        // meekomt.
        let key_value = params
            .get(key)
            .ok_or_else(|| SimulatorError::MissingParameter {
                cell: self.id.clone(),
                lexostatus: definition.name.clone(),
                parameter: key.to_string(),
            })?;

        let Some(event) = self
            .chronicles
            .latest_recording(chronicle, key, key_value, conditions, op_moment)
        else {
            return Ok(LexostatusOutcome::NotEstablished {
                reason: nothing_established(chronicle, key, key_value, conditions, op_moment),
            });
        };

        let values = definition
            .published_outputs()
            .into_iter()
            .filter_map(|output| {
                chronicle::field(&event.fields, output)
                    .map(|value| (output.to_string(), value.clone()))
            })
            .collect();
        Ok(LexostatusOutcome::Established(values))
    }
}

/// Waarom het kroniekfilter niets vond, zo precies dat het na te lopen is.
fn nothing_established(
    chronicle: &str,
    key: &str,
    key_value: &Value,
    conditions: &BTreeMap<String, Value>,
    op_moment: NaiveDate,
) -> String {
    let mut reason = format!(
        "kroniekstroom '{chronicle}' heeft op of vóór {op_moment} geen vastlegging \
         met {key} '{key_value}'"
    );
    if !conditions.is_empty() {
        let filter = conditions
            .iter()
            .map(|(field, value)| format!("{field} = {value}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(reason, " die voldoet aan {filter}");
    }
    reason
}

/// De uitkomstnamen per regeling, over alle geladen versies heen.
///
/// De engine indexeert uitkomsten alleen voor de nieuwste geladen versie,
/// terwijl een cel elke versie laadt en een vraag over een ouder moment op een
/// oudere versie landt. Voor de vraag "kent deze regeling deze uitkomst?" telt
/// daarom elke versie mee.
fn outputs_per_regulation(service: &LawExecutionService) -> BTreeMap<String, BTreeSet<String>> {
    let mut per_regulation: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for law in service.resolver().all_law_versions() {
        let known = per_regulation.entry(law.id.clone()).or_default();
        for article in &law.articles {
            let Some(execution) = article.get_execution_spec() else {
                continue;
            };
            for output in execution.output.iter().flatten() {
                known.insert(output.name.clone());
            }
        }
    }
    per_regulation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::regulation_root;

    fn config(yaml: &str) -> CellConfig {
        serde_yaml_ng::from_str(yaml).unwrap_or_else(|e| panic!("testconfig moet parsen: {e}"))
    }

    fn toeslagen() -> Cell {
        let config = config(
            r"
id: toeslagen
laws:
  - algemene_wet_inkomensafhankelijke_regelingen
chronicles:
  - stream: relaties
    key: bsn
    events:
      - op_moment: 2024-01-01
        fields:
          bsn: '999993653'
          partnerschap_type: GEEN
lexostatus_definitions:
  - name: toeslagpartnerschap
    inputs:
      - name: bsn
        type: string
    reduction:
      regulation: algemene_wet_inkomensafhankelijke_regelingen
      output: heeft_toeslagpartner
      parameters:
        bsn: $bsn
",
        );
        Cell::from_config(&config, &regulation_root())
            .unwrap_or_else(|e| panic!("cel moet op te tuigen zijn: {e}"))
    }

    fn bsn() -> BTreeMap<String, Value> {
        BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))])
    }

    fn moment() -> NaiveDate {
        date("2025-01-01")
    }

    /// Faalt luid op een onleesbare datum: deze tests draaien om de tijdas, dus
    /// een typfout mag niet stil op een standaarddatum uitkomen.
    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
    }

    /// De waarden van een antwoord, of een luide fout als er niets vastgesteld
    /// was: een test die daarop stilvalt, zou niets meer bewijzen.
    fn values(answer: &Lexostatus) -> &BTreeMap<String, Value> {
        answer.values().unwrap_or_else(|| {
            panic!(
                "verwachtte een vastgesteld feit, kreeg: {}",
                answer.not_established().unwrap_or("(onbekend)")
            )
        })
    }

    #[test]
    fn onbekende_lexostatus_noemt_wat_de_cel_wel_publiceert() {
        let err = toeslagen()
            .reduce("openstaande_vorderingen", &bsn(), moment())
            .expect_err("een niet-gepubliceerde naam hoort te falen");
        let SimulatorError::UnknownLexostatus { published, .. } = &err else {
            panic!("verwachtte UnknownLexostatus, kreeg {err}");
        };
        assert_eq!(published, "toeslagpartnerschap");
    }

    #[test]
    fn niet_gedocumenteerde_parameter_wordt_geweigerd() {
        let mut params = bsn();
        params.insert("peiljaar".to_string(), Value::Int(2025));
        let err = toeslagen()
            .reduce("toeslagpartnerschap", &params, moment())
            .expect_err("een ongedocumenteerde parameter hoort te falen");
        assert!(
            matches!(err, SimulatorError::UndocumentedParameter { .. }),
            "verwachtte UndocumentedParameter, kreeg {err}"
        );
    }

    #[test]
    fn parameter_van_het_verkeerde_type_wordt_geweigerd() {
        let params = BTreeMap::from([("bsn".to_string(), Value::Int(999_993_653))]);
        let err = toeslagen()
            .reduce("toeslagpartnerschap", &params, moment())
            .expect_err("een verkeerd getypeerde parameter hoort te falen");
        assert!(
            matches!(err, SimulatorError::ParameterType { .. }),
            "verwachtte ParameterType, kreeg {err}"
        );
    }

    /// Een cel over de zorgtoeslag, met alle feiten die die wet nodig heeft.
    ///
    /// `published` wordt letterlijk in de definitie geplakt, zodat elke test
    /// alleen het `outputs`-blok varieert.
    fn zorgtoeslag(published: &str) -> CellConfig {
        config(&format!(
            r"
id: toeslagen
laws:
  - wet_op_de_zorgtoeslag
  - algemene_wet_inkomensafhankelijke_regelingen
  - regeling_standaardpremie
chronicles:
  - stream: intake
    key: bsn
    events:
      - op_moment: 2024-11-15
        fields:
          bsn: '999993653'
          partnerschap_type: GEEN
          is_verzekerde: true
          verzamelinkomen: 79547
          buitenlands_inkomen: 0
          vermogen: 0
lexostatus_definitions:
  - name: zorgtoeslag_rechtstoestand
{published}
    inputs:
      - name: bsn
        type: string
    reduction:
      regulation: wet_op_de_zorgtoeslag
      output: heeft_recht_op_zorgtoeslag
      parameters:
        bsn: $bsn
"
        ))
    }

    #[test]
    fn een_niet_gepubliceerde_uitkomst_blijft_binnen_de_cel() {
        let cell = Cell::from_config(&zorgtoeslag(""), &regulation_root())
            .unwrap_or_else(|e| panic!("cel moet op te tuigen zijn: {e}"));
        let answer = cell
            .reduce("zorgtoeslag_rechtstoestand", &bsn(), moment())
            .unwrap_or_else(|e| panic!("de reductie moet slagen: {e}"));

        assert_eq!(
            values(&answer).get("heeft_recht_op_zorgtoeslag"),
            Some(&Value::Bool(true)),
            "de gepubliceerde uitkomst hoort in het antwoord"
        );
        assert!(
            !values(&answer).contains_key("hoogte_zorgtoeslag"),
            "de engine berekent de hoogte mee, maar de cel publiceert haar niet; kreeg {:?}",
            values(&answer)
        );
    }

    #[test]
    fn een_gepubliceerde_uitkomst_komt_er_wel_bij() {
        let cell = Cell::from_config(
            &zorgtoeslag("    outputs:\n      - hoogte_zorgtoeslag"),
            &regulation_root(),
        )
        .unwrap_or_else(|e| panic!("cel moet op te tuigen zijn: {e}"));
        let answer = cell
            .reduce("zorgtoeslag_rechtstoestand", &bsn(), moment())
            .unwrap_or_else(|e| panic!("de reductie moet slagen: {e}"));

        assert!(
            values(&answer).contains_key("hoogte_zorgtoeslag"),
            "een uitkomst die in `outputs` staat hoort in het antwoord; kreeg {:?}",
            values(&answer)
        );
    }

    #[test]
    fn een_uitkomst_die_de_regeling_niet_kent_wordt_geweigerd() {
        let err = Cell::from_config(
            &zorgtoeslag("    outputs:\n      - hoogte_huurtoeslag"),
            &regulation_root(),
        )
        .expect_err("een uitkomst die de regeling niet kent hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownOutput { .. }),
            "verwachtte UnknownOutput, kreeg {err}"
        );
    }

    /// Een bron-cel: geen wetten, één kroniek, en een lexostatus die erover
    /// filtert.
    ///
    /// `definition` wordt letterlijk ingeplakt, zodat elke test alleen het blok
    /// varieert waar ze over gaat. De tweede vastlegging laat `partner_bsn`
    /// weg: die stond in de eerste, dus de stroom kent het veld, maar deze
    /// vastlegging draagt het niet.
    fn brp(definition: &str) -> CellConfig {
        config(&format!(
            r"
id: brp
laws: []
chronicles:
  - stream: relaties
    key: bsn
    events:
      - op_moment: 2023-03-01
        fields:
          bsn: '999993653'
          partnerschap_type: HUWELIJK
          partner_bsn: '999993756'
      - op_moment: 2024-07-01
        fields:
          bsn: '999993653'
          partnerschap_type: GEEN
lexostatus_definitions:
  - name: partnerschap
    inputs:
      - name: bsn
        type: string
{definition}
"
        ))
    }

    /// Het kroniekfilter zoals de meeste tests hieronder het bedoelen.
    const PARTNERSCHAP: &str = "    outputs:
      - partnerschap_type
      - partner_bsn
    reduction:
      chronicle: relaties
      key: bsn
      latest: true";

    fn source_cell(definition: &str) -> Cell {
        Cell::from_config(&brp(definition), &regulation_root())
            .unwrap_or_else(|e| panic!("een bron-cel moet op te tuigen zijn: {e}"))
    }

    #[test]
    fn een_cel_zonder_wetten_reduceert_over_haar_eigen_kroniek() {
        let cell = source_cell(PARTNERSCHAP);

        let eerder = cell
            .reduce("partnerschap", &bsn(), date("2024-01-01"))
            .unwrap_or_else(|e| panic!("de reductie moet slagen: {e}"));
        assert_eq!(
            values(&eerder).get("partnerschap_type"),
            Some(&Value::String("HUWELIJK".to_string())),
            "op 2024-01-01 gold de vastlegging van 2023-03-01"
        );
        assert_eq!(
            values(&eerder).get("partner_bsn"),
            Some(&Value::String("999993756".to_string())),
            "de hele vastlegging telt, niet alleen het sleutelveld"
        );

        let later = cell
            .reduce("partnerschap", &bsn(), moment())
            .unwrap_or_else(|e| panic!("de reductie moet slagen: {e}"));
        assert_eq!(
            values(&later).get("partnerschap_type"),
            Some(&Value::String("GEEN".to_string())),
            "na 2024-07-01 is de latere vastlegging de laatste"
        );
        assert!(
            !values(&later).contains_key("partner_bsn"),
            "deze vastlegging draagt het veld niet, en de cel vult niets aan; kreeg {:?}",
            values(&later)
        );
    }

    #[test]
    fn niets_vastgesteld_is_een_gewoon_antwoord() {
        let answer = source_cell(PARTNERSCHAP)
            .reduce("partnerschap", &bsn(), date("2023-01-01"))
            .unwrap_or_else(|e| panic!("een moment vóór het eerste feit is geen fout: {e}"));

        let reason = answer
            .not_established()
            .unwrap_or_else(|| panic!("verwachtte 'niets vastgesteld', kreeg {answer:?}"));
        assert!(
            reason.contains("relaties") && reason.contains("2023-01-01"),
            "de reden moet zeggen waar en wanneer er niets stond, kreeg: {reason}"
        );
    }

    #[test]
    fn where_filtert_over_de_vastleggingen_en_pas_daarna_wint_de_laatste() {
        let answer = source_cell(
            "    outputs:
      - partnerschap_type
      - partner_bsn
    reduction:
      chronicle: relaties
      key: bsn
      where:
        partnerschap_type: HUWELIJK",
        )
        .reduce("partnerschap", &bsn(), moment())
        .unwrap_or_else(|e| panic!("de reductie moet slagen: {e}"));

        assert_eq!(
            values(&answer).get("partner_bsn"),
            Some(&Value::String("999993756".to_string())),
            "het filter bepaalt welke vastleggingen meedoen; van die groep wint de laatste"
        );
    }

    #[test]
    fn where_dat_niets_aantreft_stelt_niets_vast() {
        let answer = source_cell(
            "    outputs:
      - partnerschap_type
    reduction:
      chronicle: relaties
      key: bsn
      where:
        partnerschap_type: GEREGISTREERD_PARTNERSCHAP",
        )
        .reduce("partnerschap", &bsn(), moment())
        .unwrap_or_else(|e| panic!("een filter zonder treffer is geen fout: {e}"));

        let reason = answer
            .not_established()
            .unwrap_or_else(|| panic!("verwachtte 'niets vastgesteld', kreeg {answer:?}"));
        assert!(
            reason.contains("GEREGISTREERD_PARTNERSCHAP"),
            "de reden moet het filter noemen waaraan niets voldeed, kreeg: {reason}"
        );
    }

    #[test]
    fn een_onbekende_stroomnaam_wordt_geweigerd() {
        let err = Cell::from_config(
            &brp("    outputs:
      - partnerschap_type
    reduction:
      chronicle: relatis
      key: bsn"),
            &regulation_root(),
        )
        .expect_err("een typfout in de stroomnaam hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownStream { .. }),
            "verwachtte UnknownStream, kreeg {err}"
        );
    }

    #[test]
    fn een_kroniekfilter_zonder_outputs_wordt_geweigerd() {
        let err = Cell::from_config(
            &brp("    reduction:
      chronicle: relaties
      key: bsn"),
            &regulation_root(),
        )
        .expect_err("een kroniekfilter zonder `outputs` hoort te falen");
        assert!(
            matches!(err, SimulatorError::ChronicleWithoutOutputs { .. }),
            "verwachtte ChronicleWithoutOutputs, kreeg {err}"
        );
    }

    #[test]
    fn een_output_die_de_stroom_niet_kent_wordt_geweigerd() {
        let err = Cell::from_config(
            &brp("    outputs:
      - partnerschap_duur
    reduction:
      chronicle: relaties
      key: bsn"),
            &regulation_root(),
        )
        .expect_err("een uitkomst die geen vastlegging draagt hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownOutput { .. }),
            "verwachtte UnknownOutput, kreeg {err}"
        );
    }

    #[test]
    fn een_filter_op_een_onbekend_veld_wordt_geweigerd() {
        let err = Cell::from_config(
            &brp("    outputs:
      - partnerschap_type
    reduction:
      chronicle: relaties
      key: bsn
      where:
        partnerschaptype: HUWELIJK"),
            &regulation_root(),
        )
        .expect_err("een `where` op een onbekend veld hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownFilterField { .. }),
            "verwachtte UnknownFilterField, kreeg {err}"
        );
    }

    #[test]
    fn een_sleutel_zonder_gedocumenteerde_parameter_wordt_geweigerd() {
        let err = Cell::from_config(
            &brp("    outputs:
      - partnerschap_type
    reduction:
      chronicle: relaties
      key: partner_bsn"),
            &regulation_root(),
        )
        .expect_err("een sleutel die de consument niet kan meegeven hoort te falen");
        assert!(
            matches!(err, SimulatorError::ChronicleKeyWithoutParameter { .. }),
            "verwachtte ChronicleKeyWithoutParameter, kreeg {err}"
        );
    }

    #[test]
    fn een_wetsvorm_in_een_cel_zonder_wetten_wordt_geweigerd() {
        let err = Cell::from_config(
            &brp("    reduction:
      regulation: wet_basisregistratie_personen
      output: heeft_partner
      parameters:
        bsn: $bsn"),
            &regulation_root(),
        )
        .expect_err("zonder geladen regeling hoort een wetsvorm te falen");
        assert!(
            matches!(err, SimulatorError::ForeignRegulation { .. }),
            "verwachtte ForeignRegulation, kreeg {err}"
        );
    }

    #[test]
    fn reductie_over_een_vreemde_regeling_wordt_geweigerd() {
        let config = config(
            r"
id: toeslagen
laws:
  - algemene_wet_inkomensafhankelijke_regelingen
lexostatus_definitions:
  - name: vermogen
    inputs: []
    reduction:
      regulation: wet_inkomstenbelasting_2001
      output: rendementsgrondslag
",
        );
        let err = Cell::from_config(&config, &regulation_root())
            .expect_err("een reductie over een vreemde regeling hoort te falen");
        assert!(
            matches!(err, SimulatorError::ForeignRegulation { .. }),
            "verwachtte ForeignRegulation, kreeg {err}"
        );
    }
}

impl std::fmt::Debug for Cell {
    /// Toont bewust geen kronieken: ook een debugregel is een lek.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cell")
            .field("id", &self.id)
            .field("published", &self.published.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}
