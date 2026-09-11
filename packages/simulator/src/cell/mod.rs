//! De cel: een containment- en autonomiedomein, en verder niets.
//!
//! Een cel houdt kronieken, reduceert daarover en biedt het resultaat aan als
//! lexostatus. Ze houdt geen sleutels, geen bevoegdheid en geen transport —
//! dat zijn eigen assen (RFC-022 §2).
//!
//! De reductielogica woont hier, niet in de scenario-runner. De runner leest
//! configuratie en stelt vragen; wat een reductie inhoudt, weet alleen de cel.
//!
//! Twee paden, en ze zijn met opzet ongelijk:
//!
//! - **reduceren** ([`Cell::reduce`]) is publiek en zuiver: een filter of een
//!   berekening over de eigen kronieken met de eigen wetten, zonder enige weg
//!   naar een andere cel;
//! - **besluiten** (`Cell::decide`) is intern: de cel voert een eigen regeling
//!   uit en legt de uitkomst vast als decretogram. Dit is het pad waar straks
//!   een waarde van een andere cel binnenkomt, en het enige.
//!
//! Zie [`Decretogram`] voor wat een besluit vastlegt, en waarom dat het
//! RFC-013 Execution Receipt is en geen eigen formaat ernaast.

mod besluit;
mod chronicle;
mod config;

pub use besluit::{
    BesluitDefinition, BesluitInput, Decretogram, DecretogramInput, InputOrigin, BESCHIKKINGEN,
};
pub use chronicle::{ChronicleEvent, ChronicleStore, ChronicleStream, Intake};
pub use config::{CellConfig, DocumentedParameter, LexostatusDefinition, ParameterType, Reduction};

use crate::corpus;
use crate::error::{Result, SimulatorError, Subject};
use chrono::NaiveDate;
use config::{engine_parameters, CellSurface};
use regelrecht_engine::article::CompetentAuthority;
use regelrecht_engine::{ArticleBasedLaw, LawExecutionService, Value};
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
    /// De **tweede** engine over dezelfde wetten: die van het besluit-pad.
    ///
    /// Twee instanties en niet één, omdat ze niet hetzelfde mogen. De engine van
    /// [`Self::reduce`] krijgt nooit een `CellResolver` en kan de celgrens dus
    /// niet over: een cross-cel-pull vanuit een reductie is daarmee een
    /// ontbrekende capability en geen afspraak (RFC-022 §4.2, tier 3). Deze is
    /// de enige die er ooit een mag krijgen. Vandaag heeft ook zij er geen — het
    /// accepteren van een waarde van een andere cel volgt apart — dus het
    /// verschil is nu een *belofte over wie wat mag*, en de plek waar die
    /// belofte waargemaakt wordt, staat klaar.
    ///
    /// Alleen aanwezig als de cel besluit-definities heeft: een cel die niet
    /// besluit, heeft aan één engine genoeg.
    besluit_service: Option<RefCell<LawExecutionService>>,
    /// De eigen feiten. Privé, en dat is het punt.
    chronicles: ChronicleStore,
    /// De gepubliceerde lexostatussen, op naam.
    published: BTreeMap<String, LexostatusDefinition>,
    /// De besluiten die deze cel kan nemen, op naam.
    besluiten: BTreeMap<String, BesluitDefinition>,
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
    ///
    /// `fixture_fields` telt de veldnamen mee die de wereld via een `fixture`
    /// aan een stroom van deze cel toevoegt. Een stroom mag leeg opgetuigd
    /// worden en haar inhoud pas via `fixtures` krijgen — zie
    /// `scenarios/toeslagen_tijdlijn.yaml` — en dan zou `declared_fields()`
    /// alleen het sleutelveld kennen. Een kroniekfilter dat over zo'n veld
    /// praat, zou dan onterecht als typfout afgekeurd worden.
    pub fn from_config(
        config: &CellConfig,
        regulation_root: &Path,
        fixture_fields: &BTreeMap<String, BTreeSet<String>>,
    ) -> Result<Self> {
        let mut streams = config.chronicles.clone();
        if let Some(reserved) = streams
            .iter()
            .find(|stream| stream.stream == BESCHIKKINGEN)
            .map(|stream| stream.stream.clone())
        {
            return Err(SimulatorError::ReservedStream {
                cell: config.id.clone(),
                stream: reserved,
            });
        }
        if !config.besluit_definitions.is_empty() {
            streams.push(ChronicleStream {
                stream: BESCHIKKINGEN.to_string(),
                key: besluit::ZAAKKENMERK.to_string(),
                events: Vec::new(),
            });
        }
        let chronicles = ChronicleStore::from_streams(&config.id, streams)?;

        let laws = load_laws(&config.laws, regulation_root)?;
        // Twee engines over precies dezelfde wetten; zie `besluit_service`. Ze
        // worden apart opgebouwd en niet gekloond, want een `LawExecutionService`
        // draagt haar databronnen en straks haar cel-resolver met zich mee, en
        // dat is nu juist wat de twee uit elkaar houdt.
        let service = build_service(&laws)?;
        let besluit_service = if config.besluit_definitions.is_empty() {
            None
        } else {
            build_service(&laws)?
        };

        let mut declared = chronicles.declared_fields();
        for (stream, fields) in fixture_fields {
            declared
                .entry(stream.clone())
                .or_default()
                .extend(fields.iter().cloned());
        }
        if !config.besluit_definitions.is_empty() {
            declared
                .entry(BESCHIKKINGEN.to_string())
                .or_default()
                .extend(besluit::declared_fields(&config.besluit_definitions));
        }

        let surface = CellSurface {
            laws: &config.laws,
            outputs: service
                .as_ref()
                .map(outputs_per_regulation)
                .unwrap_or_default(),
            regulation_inputs: service
                .as_ref()
                .map(inputs_per_regulation)
                .unwrap_or_default(),
            stream_keys: chronicles.declared_keys(),
            streams: declared,
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

        let mut besluiten: BTreeMap<String, BesluitDefinition> = BTreeMap::new();
        for definition in &config.besluit_definitions {
            definition.validate(&config.id, &surface)?;
            if besluiten
                .insert(definition.name.clone(), definition.clone())
                .is_some()
            {
                return Err(SimulatorError::DuplicateBesluit {
                    cell: config.id.clone(),
                    name: definition.name.clone(),
                });
            }
        }

        Ok(Self {
            id: config.id.clone(),
            service: service.map(RefCell::new),
            besluit_service: besluit_service.map(RefCell::new),
            chronicles,
            published,
            besluiten,
        })
    }

    /// Leg één executogram vast in een eigen kroniekstroom.
    ///
    /// `pub(crate)` en niet `pub`: het is de eigen kroniek van de cel, dus net
    /// zomin als een consument eruit kan lezen mag hij erin schrijven. In deze
    /// crate legt alleen [`crate::World`] vast, op een moment dat de klok
    /// passeert.
    ///
    /// Vastleggen voegt toe. Een bestaand gram wordt nooit gewijzigd, dus een
    /// reductie over een eerder moment blijft na dit vastleggen exact hetzelfde.
    ///
    /// De stroom [`BESCHIKKINGEN`] kan hier niet in: daar ontstaat een gram door
    /// te *besluiten*. Zie [`Self::check_not_a_decretogram`].
    pub(crate) fn record(&mut self, stream: &str, event: ChronicleEvent) -> Result<()> {
        self.check_not_a_decretogram(stream, &event)?;
        self.record_own(stream, event)
    }

    /// Vastleggen zonder de poort op [`BESCHIKKINGEN`].
    ///
    /// Privé, en alleen voor [`Self::decide`]: dat is het enige pad dat een
    /// decretogram mág maken.
    fn record_own(&mut self, stream: &str, event: ChronicleEvent) -> Result<()> {
        self.chronicles.record(&self.id, stream, event)
    }

    /// Zou deze vastlegging in deze stroom van deze cel mogen?
    ///
    /// Alleen zodat een wereld een vastlegging bij het optuigen kan afkeuren in
    /// plaats van halverwege de tijdlijn: dezelfde toets als
    /// [`Self::record`] doet, zonder iets vast te leggen. Geeft niets prijs over
    /// de inhoud van de kroniek.
    pub(crate) fn check_recording(&self, stream: &str, event: &ChronicleEvent) -> Result<()> {
        self.check_not_a_decretogram(stream, event)?;
        self.chronicles.check_recording(&self.id, stream, event)
    }

    /// Weiger een vastlegging van buiten het besluit-pad in de stroom met
    /// decretogrammen.
    ///
    /// Dat de configuratie die stroom niet zelf mag declareren, is niet genoeg:
    /// zodra een cel besluit-definities heeft, bestaat de stroom en zou een
    /// `fixture` er een gram in kunnen zetten dat nooit langs een engine kwam —
    /// zonder receipt, zonder herkomst, en met een `intake` naar keuze. Een
    /// reductie erover zou dat niet van een besluit kunnen onderscheiden, en
    /// precies dat onderscheid is wat deze stroom waard maakt.
    fn check_not_a_decretogram(&self, stream: &str, event: &ChronicleEvent) -> Result<()> {
        if stream != BESCHIKKINGEN {
            return Ok(());
        }
        Err(SimulatorError::ReservedStreamRecording {
            cell: self.id.clone(),
            stream: stream.to_string(),
            name: event.name.clone(),
            op_moment: event.op_moment.to_string(),
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
                subject: Subject::Lexostatus,
                name: definition.name.clone(),
                regulation: regulation.to_string(),
            });
        };

        let mut service = service.borrow_mut();
        self.register_own_facts(&mut service, op_moment)?;

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

    /// Zet de eigen feiten zoals ze op dit moment waren klaar als databron.
    ///
    /// De stroom met decretogrammen blijft er met opzet buiten. Een besluit is
    /// geen feit om op te rekenen maar een uitkomst om terug te lezen; zou ze
    /// als databron meedoen, dan zou een volgende uitvoering stil op de eigen
    /// uitkomst van een eerder besluit kunnen leunen, en dan weet niemand meer
    /// of er gerekend of overgeschreven is. Terugzien doe je met een reductie
    /// over die stroom (`Cell::reduce`), en die komt niet langs de engine.
    fn register_own_facts(
        &self,
        service: &mut LawExecutionService,
        op_moment: NaiveDate,
    ) -> Result<()> {
        service.clear_data_sources();
        for stream in self.chronicles.reduce_to(op_moment) {
            if stream.stream == BESCHIKKINGEN {
                continue;
            }
            service.register_dict_source(&stream.stream, &stream.key, stream.records)?;
        }
        Ok(())
    }

    /// Het kroniekfilter: lees de laatste vastlegging over dit onderwerp.
    ///
    /// Geen engine in zicht. Wat de vastlegging draagt en de definitie
    /// publiceert, komt in het antwoord; een gepubliceerd veld dat deze
    /// vastlegging niet heeft, blijft eruit — de cel vult niets aan. Draagt de
    /// vastlegging er geen enkele, dan is er niets vastgesteld: een lege map zou
    /// op een antwoord lijken zonder er een te zijn.
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
                subject: Subject::Lexostatus,
                name: definition.name.clone(),
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

        let values: BTreeMap<String, Value> = definition
            .published_outputs()
            .into_iter()
            .filter_map(|output| {
                chronicle::field(&event.fields, output)
                    .map(|value| (output.to_string(), value.clone()))
            })
            .collect();

        // Draagt de vastlegging geen enkele gepubliceerde uitkomst, dan blijft er
        // een lege map over — en dat is precies het antwoord dat op een antwoord
        // lijkt zonder er een te zijn. Een stroom mag heterogeen zijn (het
        // optuigen eist alleen dat elke uitkomst in één of andere vastlegging
        // voorkomt), dus dit is bereikbaar met een geldige configuratie. De cel
        // zegt dan wat er aan de hand is: er was wél een vastlegging, maar niet
        // over datgene wat gevraagd werd.
        if values.is_empty() {
            return Ok(LexostatusOutcome::NotEstablished {
                reason: nothing_published(definition, chronicle, key, key_value, event.op_moment),
            });
        }

        Ok(LexostatusOutcome::Established(values))
    }

    /// Neem een besluit: voer een eigen regeling uit en leg de uitkomst vast.
    ///
    /// Dit is het derde deel van de lus — informeren, concluderen, **vastleggen**
    /// — en het enige pad in deze cel dat een gram *maakt* in plaats van er een
    /// terug te lezen. Wat er gebeurt, in deze volgorde:
    ///
    /// 1. de gedocumenteerde parameters worden gecontroleerd, net als bij een vraag;
    /// 2. de inputs worden verzameld: uit de eigen kronieken zoals ze op
    ///    `op_moment` waren, en uit de parameters van het besluit;
    /// 3. de **besluit-engine** voert de regeling uit op dat moment, dus op de
    ///    wetsversie die toen gold;
    /// 4. de uitkomst wordt als decretogram vastgelegd in de eigen stroom
    ///    [`BESCHIKKINGEN`] — één gram, met alle uitkomsten samen (RFC-022 §1.2).
    ///
    /// `pub(crate)` en niet `pub`, net als [`Self::record`]: een consument kan een
    /// cel niet laten besluiten. Dat doet de cel zelf, in deze opstelling
    /// aangestuurd door [`crate::World`] op een moment dat de klok heeft bereikt.
    ///
    /// Een input die op dit moment niet op te halen is, is een fout en geen
    /// "niets vastgesteld": een besluit dat een feit mist, hoort niet met een gat
    /// verder te rekenen en al helemaal niet vast te leggen.
    pub(crate) fn decide(
        &mut self,
        besluit: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<Decretogram> {
        let definition = self
            .besluiten
            .get(besluit)
            .ok_or_else(|| SimulatorError::UnknownBesluit {
                cell: self.id.clone(),
                requested: besluit.to_string(),
                defined: self
                    .besluiten
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
            })?
            .clone();

        definition.check_params(&self.id, params)?;
        // Vóór het ophalen en het rekenen: een zaak waarvan het kenmerk niet
        // eenduidig is, hoort er helemaal niet te komen — en dan hoeft de engine
        // er ook niet voor te draaien.
        let zaakkenmerk = definition.zaakkenmerk(&self.id, params)?;

        let inputs = self.collect_inputs(&definition, params, op_moment)?;
        let decretogram = self.execute(&definition, zaakkenmerk, inputs, op_moment)?;

        let event = decretogram.event()?;
        self.record_own(BESCHIKKINGEN, event)?;

        Ok(decretogram)
    }

    /// Verzamel de inputs van een besluit, elk met de herkomst erbij.
    ///
    /// De herkomst is geen versiering: ze gaat mee in het decretogram, zodat
    /// later te lezen is waarop besloten is en van wanneer dat feit was. Wat hier
    /// opgehaald wordt, wordt nergens anders opgeslagen — een volgend besluit
    /// haalt het opnieuw op (RFC-022: geen schaduwboekhouding).
    fn collect_inputs(
        &self,
        definition: &BesluitDefinition,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<BTreeMap<String, DecretogramInput>> {
        let mut collected: BTreeMap<String, DecretogramInput> = BTreeMap::new();
        for (input, origin) in &definition.inputs {
            let gathered = match origin {
                BesluitInput::Param { param } => {
                    // Onbereikbaar: `validate` bindt elke `param` aan een
                    // gedocumenteerde parameter, en `check_params` eist dat elke
                    // gedocumenteerde parameter meekomt.
                    let value = params.get(param).cloned().ok_or_else(|| {
                        SimulatorError::MissingParameter {
                            cell: self.id.clone(),
                            subject: Subject::Besluit,
                            name: definition.name.clone(),
                            parameter: param.clone(),
                        }
                    })?;
                    DecretogramInput {
                        value,
                        origin: InputOrigin::Parameter {
                            parameter: param.clone(),
                        },
                    }
                }
                BesluitInput::FromChronicle { chronicle, field } => {
                    self.read_own_chronicle(definition, input, chronicle, field, params, op_moment)?
                }
            };
            collected.insert(input.clone(), gathered);
        }
        Ok(collected)
    }

    /// Eén input uit een eigen kroniek: de laatste vastlegging op of vóór het
    /// moment van het besluit, over het onderwerp dat de parameters aanwijzen.
    ///
    /// Nooit uit [`BESCHIKKINGEN`]: `validate` weigert die stroom als bron bij
    /// het optuigen, want daar liggen besluiten en geen feiten.
    fn read_own_chronicle(
        &self,
        definition: &BesluitDefinition,
        input: &str,
        chronicle: &str,
        field: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<DecretogramInput> {
        // Onbereikbaar: `validate` eist dat de stroom bestaat en dat haar
        // sleutelveld een gedocumenteerde parameter is; `check_params` eist dat
        // die parameter meekomt.
        let key = self.chronicles.key_of(chronicle).unwrap_or_default();
        let missing = |reason: String| SimulatorError::BesluitInputMissing {
            cell: self.id.clone(),
            besluit: definition.name.clone(),
            input: input.to_string(),
            reason,
        };
        let key_value = params
            .get(key)
            .ok_or_else(|| missing(format!("parameter '{key}' ontbreekt in de vraag")))?;

        let no_conditions = BTreeMap::new();
        let event = self
            .chronicles
            .latest_recording(chronicle, key, key_value, &no_conditions, op_moment)
            .ok_or_else(|| {
                missing(nothing_established(
                    chronicle,
                    key,
                    key_value,
                    &no_conditions,
                    op_moment,
                ))
            })?;

        let value = chronicle::field(&event.fields, field)
            .cloned()
            .ok_or_else(|| {
                missing(format!(
                    "kroniekstroom '{chronicle}' heeft voor {key} '{key_value}' wel een \
                     vastlegging (van {}), maar die draagt veld '{field}' niet",
                    event.op_moment
                ))
            })?;

        Ok(DecretogramInput {
            value,
            origin: InputOrigin::OwnChronicle {
                chronicle: chronicle.to_string(),
                field: field.to_string(),
                recorded_op_moment: event.op_moment,
            },
        })
    }

    /// Voer de regeling uit op het moment van het besluit en maak het decretogram.
    ///
    /// De verzamelde inputs gaan als engine-parameters mee: een waarde onder de
    /// naam van een input vervangt daar haar `source`, dus het besluit leunt
    /// aantoonbaar op precies de feiten die het verzamelde. De rest van wat de
    /// regeling nodig heeft, komt uit de eigen kronieken als databron — dezelfde
    /// weg als bij een reductie, want dat is nog altijd tier 1.
    fn execute(
        &self,
        definition: &BesluitDefinition,
        zaakkenmerk: String,
        inputs: BTreeMap<String, DecretogramInput>,
        op_moment: NaiveDate,
    ) -> Result<Decretogram> {
        // Onbereikbaar: `validate` weigert een besluit over een regeling die de
        // cel niet zelf laadt, en een cel zonder engine laadt er geen enkele.
        let Some(service) = &self.besluit_service else {
            return Err(SimulatorError::ForeignRegulation {
                cell: self.id.clone(),
                subject: Subject::Besluit,
                name: definition.name.clone(),
                regulation: definition.regulation.clone(),
            });
        };

        let mut service = service.borrow_mut();
        self.register_own_facts(&mut service, op_moment)?;

        let engine_params: BTreeMap<String, Value> = inputs
            .iter()
            .map(|(name, input)| (name.clone(), input.value.clone()))
            .collect();
        let calculation_date = op_moment.format("%Y-%m-%d").to_string();
        let recorded: Vec<&str> = definition.recorded_outputs().into_iter().collect();
        let result = service.evaluate_law(
            &definition.regulation,
            &recorded,
            engine_params.clone(),
            &calculation_date,
        )?;

        let requested: Vec<String> = recorded.iter().map(|name| (*name).to_string()).collect();
        let receipt = service.build_receipt_with_outputs(
            &result,
            &engine_params,
            &calculation_date,
            &requested,
        );

        let resolver = service.resolver();
        let law = resolver.get_law_for_date(&definition.regulation, Some(op_moment));
        // Het rechtskarakter hoort bij het artikel dat de aansturende uitkomst
        // voortbrengt: díe uitkomst *is* het besluit. Een uitkomst die erbij
        // meegaat kan uit een ander artikel komen, en dat artikel zegt niets
        // over het karakter van dit besluit.
        let legal_character = resolver
            .get_article_by_output(&definition.regulation, &definition.output, Some(op_moment))
            .and_then(regelrecht_engine::Article::get_execution_spec)
            .and_then(|execution| execution.produces.as_ref())
            .and_then(|produces| produces.legal_character.clone());

        Ok(Decretogram {
            cell: self.id.clone(),
            besluit: definition.name.clone(),
            zaakkenmerk,
            op_moment,
            regulation: definition.regulation.clone(),
            regulation_valid_from: result.regulation_valid_from.clone(),
            competent_authority: law.and_then(competent_authority),
            legal_character,
            outputs: definition
                .recorded_outputs()
                .into_iter()
                .filter_map(|name| {
                    result
                        .outputs
                        .get(name)
                        .map(|value| (name.to_string(), value.clone()))
                })
                .collect(),
            inputs,
            receipt,
        })
    }
}

/// Het bevoegd gezag dat een regelingversie noemt (RFC-002).
///
/// Twee vormen in het schema, en een derde die eruitziet als de eerste: een
/// naam die met `#` begint is een **verwijzing** naar een uitkomst van de
/// regeling zelf (`competent_authority: '#bevoegd_gezag'`). Die uitkomst is niet
/// altijd als `output` gedeclareerd — vaak zet één actie haar rechtstreeks — dus
/// ze is niet via de engine op te vragen; het geladen law-model is de plek waar
/// ze wél staat.
fn competent_authority(law: &ArticleBasedLaw) -> Option<String> {
    match law.competent_authority.as_ref()? {
        CompetentAuthority::Structured { name } => Some(name.clone()),
        CompetentAuthority::String(text) => match text.strip_prefix('#') {
            Some(reference) => resolve_reference(law, reference),
            None => Some(text.clone()),
        },
    }
}

/// De letterlijke waarde die een regeling aan een eigen uitkomst toekent.
///
/// Zo werkt een `#`-verwijzing in het schema, en zo leest de rest van de
/// codebase hem ook (zie `regelrecht_corpus::source_map`): zoek de actie die
/// deze uitkomst zet en neem haar waarde.
fn resolve_reference(law: &ArticleBasedLaw, reference: &str) -> Option<String> {
    law.articles
        .iter()
        .filter_map(|article| article.get_execution_spec())
        .flat_map(|execution| execution.actions.iter().flatten())
        .find(|action| action.output.as_deref() == Some(reference))
        .and_then(|action| match action.value.as_ref()? {
            regelrecht_engine::ActionValue::Literal(Value::String(text)) => Some(text.clone()),
            _ => None,
        })
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

/// Waarom een gevonden vastlegging toch niets oplevert.
///
/// Dit is een ander verhaal dan [`nothing_established`]: er ís vastgelegd, maar
/// niet over wat deze lexostatus publiceert. De reden noemt beide, zodat de lezer
/// weet dat hij naar de velden van die vastlegging moet kijken en niet naar de
/// tijdas.
fn nothing_published(
    definition: &LexostatusDefinition,
    chronicle: &str,
    key: &str,
    key_value: &Value,
    recorded: NaiveDate,
) -> String {
    let published = definition
        .published_outputs()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "kroniekstroom '{chronicle}' heeft voor {key} '{key_value}' wel een vastlegging \
         (van {recorded}), maar die draagt geen van de gepubliceerde uitkomsten \
         ({published})"
    )
}

/// Lees elke versie van elke regeling die deze cel laadt, als YAML-tekst.
///
/// Eén keer van schijf, want er worden twee engines mee opgebouwd (zie
/// [`Cell::besluit_service`]) en die moeten per definitie dezelfde wetten
/// hebben. Twee keer lezen zou dat tot een toevalligheid maken.
fn load_laws(laws: &[String], regulation_root: &Path) -> Result<Vec<String>> {
    let mut documents = Vec::new();
    for law in laws {
        documents.extend(corpus::regulation_versions(regulation_root, law)?);
    }
    Ok(documents)
}

/// Bouw een engine met deze wetten, of geen engine als er geen wetten zijn.
///
/// `laws: []` levert `None`: een bron-cel draait geen engine, en dat is de vorm
/// van RFC-022 §2 — de engine is een component dat in een cel kán draaien.
fn build_service(documents: &[String]) -> Result<Option<LawExecutionService>> {
    if documents.is_empty() {
        return Ok(None);
    }
    let mut service = LawExecutionService::new();
    for document in documents {
        service.load_law(document)?;
    }
    Ok(Some(service))
}

/// De namen die een regeling als parameter of input declareert, over alle
/// geladen versies heen.
///
/// Hiermee valt bij het optuigen te toetsen of een besluit de engine iets
/// aanlevert dat de regeling ook echt vraagt. Alle versies tellen mee, om
/// dezelfde reden als bij [`outputs_per_regulation`]: een besluit over een
/// ouder moment landt op een oudere versie.
fn inputs_per_regulation(service: &LawExecutionService) -> BTreeMap<String, BTreeSet<String>> {
    let mut per_regulation: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for law in service.resolver().all_law_versions() {
        let known = per_regulation.entry(law.id.clone()).or_default();
        for article in &law.articles {
            let Some(execution) = article.get_execution_spec() else {
                continue;
            };
            for parameter in execution.parameters.iter().flatten() {
                known.insert(parameter.name.clone());
            }
            for input in execution.input.iter().flatten() {
                known.insert(input.name.clone());
            }
        }
    }
    per_regulation
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

    /// Geen fixtures: deze tests tuigen een cel rechtstreeks op, buiten een
    /// wereld om, dus er is niets dat een stroom extra velden aanreikt.
    fn no_fixtures() -> BTreeMap<String, BTreeSet<String>> {
        BTreeMap::new()
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
      - name: relatie_gewijzigd
        intake: levering
        recording_actor: toeslagen
        op_moment: 2024-01-01
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
        Cell::from_config(&config, &regulation_root(), &no_fixtures())
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
  - stream: inkomensleveringen
    key: bsn
    events:
      - name: inkomenslevering
        intake: levering
        recording_actor: toeslagen
        grondslag: jaarlijkse inkomenslevering
        op_moment: 2024-11-15
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
        let cell = Cell::from_config(&zorgtoeslag(""), &regulation_root(), &no_fixtures())
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
            &no_fixtures(),
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
            &no_fixtures(),
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
      - name: relatie_gewijzigd
        intake: levering
        recording_actor: brp
        op_moment: 2023-03-01
        fields:
          bsn: '999993653'
          partnerschap_type: HUWELIJK
          partner_bsn: '999993756'
      - name: relatie_gewijzigd
        intake: levering
        recording_actor: brp
        op_moment: 2024-07-01
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
        Cell::from_config(&brp(definition), &regulation_root(), &no_fixtures())
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
    fn een_vastlegging_zonder_gepubliceerd_veld_levert_geen_lege_map_op() {
        // Een stroom mag heterogeen zijn: het optuigen eist alleen dat elke
        // gepubliceerde uitkomst in één of andere vastlegging voorkomt. De
        // laatste vastlegging hier draagt alleen de sleutel, dus er valt niets
        // te publiceren — en een leeg antwoord dat op een antwoord lijkt, is
        // precies wat `LexostatusOutcome` moet voorkomen.
        let cell = Cell::from_config(
            &config(
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
      - name: relatie_gewijzigd
        intake: levering
        recording_actor: brp
        op_moment: 2024-07-01
        fields:
          bsn: '999993653'
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
",
            ),
            &regulation_root(),
            &no_fixtures(),
        )
        .unwrap_or_else(|e| panic!("een bron-cel moet op te tuigen zijn: {e}"));

        let answer = cell
            .reduce("partnerschap", &bsn(), moment())
            .unwrap_or_else(|e| panic!("de reductie moet slagen: {e}"));

        let reason = answer.not_established().unwrap_or_else(|| {
            panic!("verwachtte 'niets vastgesteld', kreeg {:?}", answer.outcome)
        });
        assert!(
            reason.contains("2024-07-01") && reason.contains("partnerschap_type"),
            "de reden moet de vastlegging noemen die niets publiceert, kreeg: {reason}"
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
            &no_fixtures(),
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
            &no_fixtures(),
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
            &no_fixtures(),
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
            &no_fixtures(),
        )
        .expect_err("een `where` op een onbekend veld hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownFilterField { .. }),
            "verwachtte UnknownFilterField, kreeg {err}"
        );
    }

    #[test]
    fn een_where_die_naar_een_parameter_verwijst_wordt_geweigerd() {
        let err = Cell::from_config(
            &brp("    outputs:
      - partnerschap_type
    reduction:
      chronicle: relaties
      key: bsn
      where:
        partner_bsn: $bsn"),
            &regulation_root(),
            &no_fixtures(),
        )
        .expect_err("`where` kent geen `$`-verwijzingen, dus dit hoort te falen");
        assert!(
            matches!(err, SimulatorError::FilterValueReference { .. }),
            "verwachtte FilterValueReference, kreeg {err}"
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
            &no_fixtures(),
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
            &no_fixtures(),
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
        let err = Cell::from_config(&config, &regulation_root(), &no_fixtures())
            .expect_err("een reductie over een vreemde regeling hoort te falen");
        assert!(
            matches!(err, SimulatorError::ForeignRegulation { .. }),
            "verwachtte ForeignRegulation, kreeg {err}"
        );
    }

    /// Een cel die één besluit kan nemen over haar eigen zorgtoeslagwet.
    ///
    /// `besluit` wordt letterlijk ingeplakt, zodat elke test hieronder alleen
    /// het blok varieert waar ze over gaat. De inkomenslevering ligt op
    /// 2024-11-15: een besluit dáárvoor mist zijn input, en dat is waar de
    /// tijdas in dit pad zichtbaar wordt.
    fn besluitende_cel(besluit: &str) -> CellConfig {
        config(&format!(
            r"
id: toeslagen
laws:
  - wet_op_de_zorgtoeslag
  - algemene_wet_inkomensafhankelijke_regelingen
  - regeling_standaardpremie
chronicles:
  - stream: inkomensleveringen
    key: bsn
    events:
      - name: inkomenslevering
        intake: levering
        recording_actor: toeslagen
        op_moment: 2024-11-15
        fields:
          bsn: '999993653'
          partnerschap_type: GEEN
          is_verzekerde: true
          verzamelinkomen: 79547
          buitenlands_inkomen: 0
          vermogen: 0
besluit_definitions:
  - name: zorgtoeslag_vaststelling
{besluit}
"
        ))
    }

    /// Het besluit zoals de meeste tests hieronder het bedoelen.
    const VASTSTELLING: &str = "    regulation: wet_op_de_zorgtoeslag
    output: heeft_recht_op_zorgtoeslag
    outputs:
      - hoogte_zorgtoeslag
    zaakkenmerk: 'zorgtoeslag/{bsn}'
    params:
      - name: bsn
        type: string
    inputs:
      bsn:
        param: bsn
      is_verzekerde:
        from_chronicle: inkomensleveringen
        field: is_verzekerde";

    fn besluitende_toeslagen(besluit: &str) -> Cell {
        Cell::from_config(
            &besluitende_cel(besluit),
            &regulation_root(),
            &no_fixtures(),
        )
        .unwrap_or_else(|e| panic!("een besluitende cel moet op te tuigen zijn: {e}"))
    }

    /// Eén besluit is één gram, met alles wat samen ontstond erin.
    ///
    /// Dat is de elementariteit van RFC-022 §1.2: wat tegelijk ontstaat, wordt
    /// samen vastgelegd. Twee grammen — één per uitkomst — zou van één besluit
    /// twee besluiten maken, en dan is niet meer te zeggen welk bedrag bij welk
    /// recht hoorde.
    #[test]
    fn een_besluit_legt_precies_een_decretogram_vast() {
        let mut cell = besluitende_toeslagen(VASTSTELLING);
        let gram = cell
            .decide("zorgtoeslag_vaststelling", &bsn(), moment())
            .unwrap_or_else(|e| panic!("het besluit moet genomen kunnen worden: {e}"));

        assert_eq!(
            cell.chronicles.len_of(BESCHIKKINGEN),
            Some(1),
            "één besluit hoort precies één gram vast te leggen"
        );
        assert_eq!(gram.zaakkenmerk, "zorgtoeslag/999993653");
        assert_eq!(gram.op_moment, moment());
        assert_eq!(
            gram.outputs.keys().collect::<Vec<_>>(),
            ["heeft_recht_op_zorgtoeslag", "hoogte_zorgtoeslag"],
            "beide uitkomsten horen in hetzelfde gram te staan"
        );
        assert_eq!(
            gram.legal_character.as_deref(),
            Some("BESCHIKKING"),
            "het rechtskarakter komt uit de wet, niet uit de cel"
        );
        assert_eq!(
            gram.competent_authority.as_deref(),
            Some("Dienst Toeslagen"),
            "een `#`-verwijzing naar een eigen uitkomst hoort opgelost te worden"
        );
        assert_eq!(
            gram.regulation_valid_from.as_deref(),
            Some("2025-01-01"),
            "de versie die op het moment van het besluit (2025-01-01) gold"
        );
    }

    /// Het gram draagt zijn eigen inputs, met de herkomst erbij.
    ///
    /// Zonder die herkomst is een besluit niet terug te lezen: dan staat er wel
    /// een waarde in, maar niet van wanneer ze was of wie haar leverde.
    #[test]
    fn het_decretogram_draagt_zijn_inputs_met_herkomst() {
        let mut cell = besluitende_toeslagen(VASTSTELLING);
        let gram = cell
            .decide("zorgtoeslag_vaststelling", &bsn(), moment())
            .unwrap_or_else(|e| panic!("het besluit moet genomen kunnen worden: {e}"));

        let uit_de_kroniek = gram
            .inputs
            .get("is_verzekerde")
            .unwrap_or_else(|| panic!("de input uit de kroniek hoort in het gram te staan"));
        assert_eq!(uit_de_kroniek.value, Value::Bool(true));
        assert_eq!(
            uit_de_kroniek.origin,
            InputOrigin::OwnChronicle {
                chronicle: "inkomensleveringen".to_string(),
                field: "is_verzekerde".to_string(),
                recorded_op_moment: date("2024-11-15"),
            },
            "de herkomst noemt de stroom, het veld en het moment van de vastlegging"
        );

        let uit_de_vraag = gram
            .inputs
            .get("bsn")
            .unwrap_or_else(|| panic!("de parameter hoort in het gram te staan"));
        assert_eq!(
            uit_de_vraag.origin,
            InputOrigin::Parameter {
                parameter: "bsn".to_string()
            }
        );
    }

    /// Een besluit dat een feit niet heeft, rekent niet door en legt niets vast.
    ///
    /// Anders dan een reductie: "niets vastgesteld" is een geldig *antwoord*,
    /// maar geen geldige grondslag om op te besluiten.
    #[test]
    fn een_besluit_zonder_zijn_feiten_faalt_en_legt_niets_vast() {
        let mut cell = besluitende_toeslagen(VASTSTELLING);
        let err = cell
            .decide("zorgtoeslag_vaststelling", &bsn(), date("2024-01-01"))
            .expect_err("vóór de levering is er geen feit om op te besluiten");
        assert!(
            matches!(err, SimulatorError::BesluitInputMissing { .. }),
            "verwachtte BesluitInputMissing, kreeg {err}"
        );
        assert_eq!(
            cell.chronicles.len_of(BESCHIKKINGEN),
            Some(0),
            "een besluit dat niet doorging, hoort geen gram achter te laten"
        );
    }

    /// Twee besluiten op één dag over dezelfde zaak: beide grammen blijven, en
    /// het laatstgenomen besluit is wat een reductie oplevert.
    ///
    /// De dag is de fijnste korrel van deze tijdas, dus op de tijdas zelf zijn ze
    /// niet uit elkaar te houden; de volgorde van vastleggen beslist dan. Dat een
    /// kroniek groeit en niets vervangt, blijft daarbij overeind: het eerste gram
    /// staat er nog.
    #[test]
    fn twee_besluiten_op_een_dag_leveren_het_laatstgenomen_besluit() {
        let mut cell = besluitende_toeslagen(VASTSTELLING);
        for _ in 0..2 {
            cell.decide("zorgtoeslag_vaststelling", &bsn(), moment())
                .unwrap_or_else(|e| panic!("het besluit moet genomen kunnen worden: {e}"));
        }

        assert_eq!(
            cell.chronicles.len_of(BESCHIKKINGEN),
            Some(2),
            "een tweede besluit vervangt het eerste niet; de kroniek groeit"
        );

        let laatste = cell
            .chronicles
            .latest_recording(
                BESCHIKKINGEN,
                besluit::ZAAKKENMERK,
                &Value::String("zorgtoeslag/999993653".to_string()),
                &BTreeMap::new(),
                moment(),
            )
            .unwrap_or_else(|| panic!("er liggen twee grammen over deze zaak"));
        let laatst_vastgelegd = cell
            .chronicles
            .last_recording(BESCHIKKINGEN)
            .unwrap_or_else(|| panic!("de stroom heeft vastleggingen"));
        assert!(
            std::ptr::eq(laatste, laatst_vastgelegd),
            "op één dag beslist de volgorde van vastleggen, en de laatste wint"
        );
    }

    #[test]
    fn een_onbekend_besluit_noemt_wat_de_cel_wel_kent() {
        let err = besluitende_toeslagen(VASTSTELLING)
            .decide("zorgtoeslag_terugvordering", &bsn(), moment())
            .expect_err("een besluit dat niet gedefinieerd is hoort te falen");
        let SimulatorError::UnknownBesluit { defined, .. } = &err else {
            panic!("verwachtte UnknownBesluit, kreeg {err}");
        };
        assert_eq!(defined, "zorgtoeslag_vaststelling");
    }

    #[test]
    fn een_input_die_de_regeling_niet_declareert_wordt_geweigerd() {
        let err = Cell::from_config(
            &besluitende_cel(
                "    regulation: wet_op_de_zorgtoeslag
    output: heeft_recht_op_zorgtoeslag
    zaakkenmerk: 'zorgtoeslag/{bsn}'
    params:
      - name: bsn
        type: string
    inputs:
      is_verzekert:
        from_chronicle: inkomensleveringen
        field: is_verzekerde",
            ),
            &regulation_root(),
            &no_fixtures(),
        )
        .expect_err("een input die de regeling niet kent hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownRegulationInput { .. }),
            "verwachtte UnknownRegulationInput, kreeg {err}"
        );
    }

    /// Een besluit leest geen besluit.
    ///
    /// Zodra een cel besluit-definities heeft, is `beschikkingen` een gewone
    /// stroom met gewone velden, en zou een tweede besluit een veld van een
    /// eerder decretogram als "eigen feit" kunnen binnenhalen. Dat is dezelfde
    /// schaduwboekhouding die `register_own_facts` aan de kant van de engine al
    /// buiten de deur houdt, langs de andere weg.
    #[test]
    fn een_besluit_kan_geen_eerder_besluit_als_input_lezen() {
        let err = Cell::from_config(
            &besluitende_cel(
                "    regulation: wet_op_de_zorgtoeslag
    output: heeft_recht_op_zorgtoeslag
    zaakkenmerk: 'zorgtoeslag/{bsn}'
    params:
      - name: bsn
        type: string
    inputs:
      is_verzekerde:
        from_chronicle: beschikkingen
        field: heeft_recht_op_zorgtoeslag",
            ),
            &regulation_root(),
            &no_fixtures(),
        )
        .expect_err("een besluit dat uit de beschikkingen leest hoort te falen");
        assert!(
            matches!(err, SimulatorError::DecretogramAsBesluitInput { .. }),
            "verwachtte DecretogramAsBesluitInput, kreeg {err}"
        );
    }

    #[test]
    fn de_stroom_voor_decretogrammen_is_voorbehouden() {
        let err = Cell::from_config(
            &config(
                r"
id: toeslagen
laws: []
chronicles:
  - stream: beschikkingen
    key: zaakkenmerk
",
            ),
            &regulation_root(),
            &no_fixtures(),
        )
        .expect_err("de stroom van het besluit-pad hoort niet zelf gedeclareerd te worden");
        assert!(
            matches!(err, SimulatorError::ReservedStream { .. }),
            "verwachtte ReservedStream, kreeg {err}"
        );
    }

    /// Een decretogram is geen feit om op te rekenen.
    ///
    /// De stroom met besluiten gaat niet als databron naar de engine. Zou ze dat
    /// wel doen, dan zou een volgende uitvoering stil op de uitkomst van een
    /// eerder besluit kunnen leunen — en dan is niet meer te zeggen of er
    /// gerekend of overgeschreven is.
    #[test]
    fn de_eigen_besluiten_gaan_niet_als_databron_naar_de_engine() {
        let mut cell = besluitende_toeslagen(VASTSTELLING);
        cell.decide("zorgtoeslag_vaststelling", &bsn(), moment())
            .unwrap_or_else(|e| panic!("het besluit moet genomen kunnen worden: {e}"));

        let Some(service) = &cell.service else {
            panic!("deze cel laadt wetten en heeft dus een engine");
        };
        let mut service = service.borrow_mut();
        cell.register_own_facts(&mut service, date("2025-01-01"))
            .unwrap_or_else(|e| panic!("de eigen feiten moeten klaargezet kunnen worden: {e}"));
        assert!(
            !service.list_data_sources().contains(&BESCHIKKINGEN),
            "de stroom met decretogrammen hoort geen databron te zijn, kreeg {:?}",
            service.list_data_sources()
        );
    }

    /// Een resolver die niets weet; hij hoeft alleen te bestaan.
    struct GeenPeers;

    impl regelrecht_engine::CellResolver for GeenPeers {
        fn resolve(
            &self,
            _cell_id: &str,
            _output: &str,
            _parameters: &BTreeMap<String, Value>,
            _reference_date: &str,
        ) -> regelrecht_engine::Result<Option<Value>> {
            Ok(None)
        }
    }

    /// De haak voor tier 3 zit op de besluit-engine, en nergens anders.
    ///
    /// Vandaag heeft geen van beide engines een resolver — accepteren van een
    /// waarde van een andere cel volgt apart — en dit is de test die vastlegt
    /// waar die ooit terechtkomt. Dat de twee losse instanties zijn, is wat het
    /// verschil afdwingbaar maakt: een resolver op de ene raakt de andere niet.
    #[test]
    fn alleen_de_besluit_engine_kan_de_celgrens_over() {
        let cell = besluitende_toeslagen(VASTSTELLING);
        let (Some(reduce), Some(besluit)) = (&cell.service, &cell.besluit_service) else {
            panic!("een besluitende cel met wetten heeft beide engines");
        };
        assert!(
            reduce.borrow().cell_ids().is_empty() && besluit.borrow().cell_ids().is_empty(),
            "zonder resolver bestaat tier 3 niet, voor geen van beide engines"
        );

        besluit
            .borrow_mut()
            .set_cell_resolver(["brp"], std::rc::Rc::new(GeenPeers))
            .unwrap_or_else(|e| panic!("de besluit-engine hoort een resolver te accepteren: {e}"));

        assert_eq!(
            besluit.borrow().cell_ids(),
            ["brp"],
            "de besluit-engine is de plek waar de celgrens open kan"
        );
        assert!(
            reduce.borrow().cell_ids().is_empty(),
            "en de reduce-engine blijft daar buiten: een reductie raakt geen andere cel"
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
