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
use regelrecht_engine::{LawExecutionService, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
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
    /// De waarden die de reductie opleverde.
    ///
    /// Uitsluitend de uitkomsten die de definitie publiceert. De engine levert
    /// bij een gevraagde uitkomst ook wat er causaal mee meekomt; berekend is
    /// niet gepubliceerd, dus dat blijft binnen de cel.
    pub values: BTreeMap<String, Value>,
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
    /// Engine met uitsluitend de eigen wetten van deze cel geladen.
    ///
    /// In een `RefCell`, omdat elke reductie de zichtbare feiten opnieuw op het
    /// gevraagde moment zet; naar buiten toe blijft bevragen een leesactie.
    service: RefCell<LawExecutionService>,
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
    pub fn from_config(config: &CellConfig, regulation_root: &Path) -> Result<Self> {
        let mut service = LawExecutionService::new();
        for law in &config.laws {
            for document in corpus::regulation_versions(regulation_root, law)? {
                service.load_law(&document)?;
            }
        }

        let known_outputs = outputs_per_regulation(&service);

        let mut published: BTreeMap<String, LexostatusDefinition> = BTreeMap::new();
        for definition in &config.lexostatus_definitions {
            definition.validate(&config.id, &config.laws, &known_outputs)?;
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
            service: RefCell::new(service),
            chronicles: ChronicleStore::from_streams(&config.id, config.chronicles.clone())?,
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
    /// deze cel zijn vastgelegd, bestaan voor dit antwoord niet.
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

        let bound = definition.bind(&self.id, params)?;

        let mut service = self.service.borrow_mut();
        service.clear_data_sources();
        for stream in self.chronicles.reduce_to(op_moment) {
            service.register_dict_source(&stream.stream, &stream.key, stream.records)?;
        }

        let result = service.evaluate_law_output(
            &definition.reduction.regulation,
            &definition.reduction.output,
            bound,
            &op_moment.format("%Y-%m-%d").to_string(),
        )?;

        Ok(Lexostatus {
            cell: self.id.clone(),
            name: definition.name.clone(),
            op_moment,
            values: definition.project(result.outputs),
        })
    }
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
        NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap_or_else(|| panic!("2025-01-01 moet een geldige datum zijn"))
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
            answer.values.get("heeft_recht_op_zorgtoeslag"),
            Some(&Value::Bool(true)),
            "de gepubliceerde uitkomst hoort in het antwoord"
        );
        assert!(
            !answer.values.contains_key("hoogte_zorgtoeslag"),
            "de engine berekent de hoogte mee, maar de cel publiceert haar niet; kreeg {:?}",
            answer.values
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
            answer.values.contains_key("hoogte_zorgtoeslag"),
            "een uitkomst die in `outputs` staat hoort in het antwoord; kreeg {:?}",
            answer.values
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
