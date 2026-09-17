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
//!   uit en legt de uitkomst vast als decretogram. Dit is het enige pad waar een
//!   waarde van een andere cel binnenkomt — *geaccepteerd*, met haar herkomst in
//!   het gram (invariant I5).
//!
//! Ook op dat tweede pad reikt de cel niet zelf over haar grens. Ze zegt wat ze
//! van een ander nodig heeft ([`Cell::acceptance_requests`]) en krijgt het
//! aangereikt; het ophalen gebeurt buiten de cel, want een veiligheidscontext en
//! een transport houdt ze niet. Zie `accept.rs` en [`crate::World::decide`].
//!
//! Zie [`Decretogram`] voor wat een besluit vastlegt, en waarom dat het
//! RFC-013 Execution Receipt is en geen eigen formaat ernaast.

mod besluit;
mod chronicle;
mod config;
mod extensions;
pub(crate) mod openstaand;
mod reductie;
mod schema;

pub use besluit::{
    AcceptanceRequest, Afwijzingsgrond, Bekendmaking, BesluitDefinition, BesluitInput,
    ChronicleSource, Decretogram, DecretogramInput, ExecutedRegulation, HookHerkomst,
    HookNietUitgevoerd, InputOrigin, NietsTeBetalen, ObligationDefinition, ObligationDue,
    ObligationKind, ObligationOrigin, ObsoleteField, RichtingBijNegatief, Schedule, ScheduleOrigin,
    StageUitkomstHerkomst, TermijnenVervallen, Vervanging, WachtendeVerplichting, AFWIJZING,
    BESCHIKKING, BESCHIKKINGEN, BETALINGEN, DECISION_TYPE, STAGE, STAGE_BEKENDMAKING,
    STAGE_BESLUIT, ZAAKKENMERK,
};
pub(crate) use besluit::{BesluitGram, DeclaredObligations, ObligationScope};
// Eén sjabloonlezer voor het zaakkenmerk én voor de vragen van het portaal: een
// `{veld}` hoort op beide plekken hetzelfde te betekenen.
pub(crate) use besluit::{closing_braces_match, Template};
// De vaste velden van een decretogram, voor het beeld van de wereld: dat moet een
// uitkomst van een besluit van een vast veld kunnen onderscheiden om de herkomst
// van elke waarde te kunnen noemen. `pub(crate)`, want het is geen contract naar
// buiten — wat een gram draagt, staat in [`Decretogram`].
pub(crate) use besluit::{
    beschikkingen_fields, nakoming_schema, recorded_input, BESLUIT, COMPETENT_AUTHORITY, INPUTS,
    RECEIPT, REGULATION,
};
// Het formulier van een actie wordt tegen dezelfde toets gehouden als de
// parameters van een lexostatus of een besluit: precies wat gedocumenteerd is,
// niets erbij en niets van het verkeerde type.
pub use chronicle::{
    ChronicleEvent, ChronicleStore, ChronicleStream, GebeurtenisSchema, Intake, SchemaVeld,
};
// De toets dat een stroom het ingebouwde schema van een betaling bijhoudt: de
// wereld stelt hem, want alleen zij weet welke cel welke verplichting nakomt.
pub(crate) use chronicle::uncovered;
pub(crate) use config::{check_documented_params, check_parameter_value, check_prefill_values};
pub use config::{
    AcceptedSource, Aggregate, CellConfig, DocumentedParameter, LexostatusDefinition, Opschorting,
    ParameterType, Prefill, Reduction,
};
pub use reductie::{
    GebruiktGram, GebruikteInput, Gemist, InputHerkomst, Kroniekfilter, Openstaandvorm, Reductie,
    ReductieVorm, Regel, Wetsvorm,
};
// Het schema van het decretogram dat een besluit kan voortbrengen: per veld het
// type en het lexogram dat het declareert. Deel van het beeld van de wereld, dus
// publiek — zie [`schema`] voor wat de vier herkomsten betekenen.
pub use schema::{DecretogramField, Herkomst, LexogramRef};

use crate::corpus;
use crate::error::{Result, SimulatorError, Subject};
use crate::values::amount;
use chronicle::ChronicleView;
use chrono::NaiveDate;
use config::{binding_name, engine_parameters, CellSurface, ObligationsPerOutput};
use extensions::ChronolexBlock;
use regelrecht_engine::article::CompetentAuthority;
use regelrecht_engine::{
    Article, ArticleBasedLaw, ArticleResult, CellResolver, EngineError, ExecutionOutcome,
    InputProvenance, LawExecutionService, OutputProvenance, RuleResolver, SkippedHook, StageState,
    TraceBuilder, Value,
};
use rust_decimal::Decimal;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::rc::Rc;

/// Hoe het met de bekendmaking van één besluit staat.
///
/// Drie standen, en ze zijn alle drie **afgeleid** uit de kroniek van de cel: er
/// ligt nog geen besluit, er ligt er een dat nog bekendgemaakt moet worden, of
/// het is al gebeurd. Dat is wat "kan deze actie nu?" hier betekent — geen
/// voorwaarde die in het wereldbestand opgeschreven staat, maar een stand van
/// zaken die uit de grammen zelf volgt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BekendmakingStand {
    /// Er ligt geen gram van de stage BESLUIT: er valt niets bekend te maken.
    GeenBesluit,
    /// Er ligt al een gram van de stage BEKENDMAKING dat naar dit besluit wijst.
    AlBekendgemaakt {
        /// De zaak waarover het besluit ging.
        zaakkenmerk: String,
        /// Het moment van het besluit dat al bekendgemaakt is.
        besluit_op_moment: NaiveDate,
        /// Wanneer die bekendmaking gebeurde.
        op_moment: NaiveDate,
    },
    /// Er ligt een besluit dat nog bekendgemaakt kan worden.
    TeDoen {
        /// De zaak waar het om gaat.
        zaakkenmerk: String,
        /// De plek van het besluit-gram in [`BESCHIKKINGEN`].
        plek: usize,
        /// Het moment van dat besluit.
        besluit_op_moment: NaiveDate,
    },
}

impl BekendmakingStand {
    /// Kan er nu bekendgemaakt worden?
    pub(crate) fn kan_nu(&self) -> bool {
        matches!(self, Self::TeDoen { .. })
    }

    /// Waarom het nu niet kan, in woorden — met het gram dat ontbreekt of het
    /// gram dat er al ligt.
    ///
    /// Een lege tekst als het wél kan: wie deze reden toont, toont haar alleen
    /// bij een weigering.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::GeenBesluit => {
                "er ligt in 'beschikkingen' geen gram van de stage BESLUIT over dit besluit"
                    .to_string()
            }
            Self::AlBekendgemaakt {
                zaakkenmerk,
                besluit_op_moment,
                op_moment,
            } => format!(
                "het besluit van {besluit_op_moment} over zaak '{zaakkenmerk}' is op \
                 {op_moment} al bekendgemaakt: er ligt een gram van de stage BEKENDMAKING \
                 dat ernaar wijst"
            ),
            Self::TeDoen { .. } => String::new(),
        }
    }
}

/// Een hook die bij een besluit vuurt en een verplichte parameter vraagt die er
/// niet is; zie [`Cell::hooks_zonder_input`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HookZonderInput {
    /// De regeling van de hook, bij `$id`.
    pub(crate) regulation: String,
    /// Het artikel van de hook.
    pub(crate) article: String,
    /// De parameter die er niet is.
    pub(crate) input: String,
}

/// De stage BEKENDMAKING zoals de procedure van een besluit haar declareert.
///
/// Zie [`Cell::bekendmaking_procedure`].
#[derive(Debug, Clone)]
pub(crate) struct BekendmakingProcedure {
    /// De procedure die voor het besluit geldt.
    pub(crate) procedure_id: String,
    /// Het formulier: de `requires` van de stage, als gedocumenteerde velden.
    pub(crate) form: Vec<DocumentedParameter>,
    /// Het veld dat de dag van de bekendmaking draagt: het eerste datumveld.
    pub(crate) datumveld: String,
}

/// Wat één uitgevoerde stage opleverde.
///
/// De uitkomsten van de hooks die op die stage vuurden en van de eigen regeling,
/// met hun herkomst, en het receipt van de uitvoering. Bij elkaar, want ze komen
/// uit dezelfde uitvoering: wie ze los doorgeeft, kan een uitkomst met de
/// herkomst van een andere naar buiten brengen.
struct StageUitkomst {
    /// Wat de hooks en de eigen regeling opleverden.
    outputs: BTreeMap<String, Value>,
    /// Waar elke uitkomst van een hook vandaan komt.
    hooks: Vec<HookHerkomst>,
    /// Waar elke uitkomst van de eigen regeling vandaan komt.
    stage_uitkomsten: Vec<StageUitkomstHerkomst>,
    /// De hooks die vuurden maar niet konden draaien.
    hooks_niet_uitgevoerd: Vec<HookNietUitgevoerd>,
    /// De `valid_from` van de regelingversie die gold.
    regulation_valid_from: Option<String>,
    /// Het receipt van de stage-uitvoering.
    receipt: regelrecht_engine::receipt::ExecutionReceipt,
}

impl StageUitkomst {
    /// Wat deze stage opleverde, als leesbare opsomming voor een melding.
    fn listing(&self) -> String {
        if self.outputs.is_empty() {
            return "de stage leverde geen enkele uitkomst".to_string();
        }
        format!(
            "de stage leverde: {}",
            self.outputs
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Het antwoord van een cel: de rechtstoestand vanuit een gevraagd perspectief,
/// op de feiten die in die cel bekend zijn.
///
/// `Serialize` hoort erbij omdat een antwoord over een celgrens in het beeld van
/// de wereld komt te staan (zie [`crate::Snapshot`]) en daar precies zo hoort te
/// verschijnen als de cel het gaf — inclusief "niets vastgesteld", dat een
/// antwoord is en geen leeg antwoord.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Lexostatus {
    /// De cel die geantwoord heeft.
    pub cell: String,
    /// De gepubliceerde naam die gevraagd werd.
    pub name: String,
    /// Het moment waarop gevraagd is; het antwoord geldt op dat moment.
    pub op_moment: NaiveDate,
    /// Wat de cel op dat moment vond.
    pub outcome: LexostatusOutcome,
    /// Hoe ze daaraan kwam: welke gegevens ze las en hoe ze die reduceerde.
    ///
    /// Elk antwoord van [`Cell::reduce`] draagt het. `None` staat er alleen waar
    /// een antwoord niet uit een reductie komt — een gesimuleerde peer in een
    /// test die een antwoord nabootst zonder kroniek eronder. Een uitkomst
    /// zonder herkomst is geen reductie, en dat hoort te zien te zijn.
    pub reductie: Option<Reductie>,
}

/// De twee antwoorden die een reductie kan opleveren.
///
/// "Niets vastgesteld" is er één van. Een cel die op het gevraagde moment geen
/// feit had, heeft niet gefaald en is niet stuk; ze heeft een antwoord dat een
/// consument moet kunnen onderscheiden van een antwoord met waarden. Een lege
/// map zou dat onderscheid verstoppen, want die lijkt op een antwoord.
///
/// Geserialiseerd blijven de twee daarom uit elkaar: `{"established": {…}}`
/// tegenover `{"not_established": {"reason": "…"}}`. Een vorm waarin ze
/// samenvallen — een lege map, een `null` — zou het onderscheid dat deze
/// opstelling maakt precies bij de grens naar buiten weer weggooien.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
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

/// Welke cel er in deze wereld onder welke **naam** nakomt.
///
/// Uitvoering en geen recht: dát er nagekomen moet worden staat in de regeling,
/// welk systeem de betaling doet is een inrichtingskeuze van de organisatie die
/// haar uitvoert. Daarom bindt het **wereldbestand** dit en niet het lexogram.
///
/// De naam is de schuldenaar van een verplichting: het bevoegd gezag dat de wet
/// aanwijst, of de partij die een besluit-parameter benoemt. Twee wegen ernaartoe,
/// en in deze volgorde:
///
/// 1. **`komt_na`**: een cel zegt zelf voor welke naam zij nakomt. Zo betaalt een
///    betaalsysteem namens een bestuursorgaan dat zelf geen cel is.
/// 2. **De identiteit van een cel**: is de schuldenaar een partij die als cel in
///    de wereld bestaat, dan betaalt die cel zelf.
///
/// De eerste gaat vóór, en dat moet ook: een bestuursorgaan dat als cel besluit
/// heeft doorgaans een ander systeem dat voor haar betaalt, en dat systeem zegt
/// dat met `komt_na`. Zou de identiteit voorgaan, dan betaalde de besluitende cel
/// zichzelf terwijl het wereldbestand iets anders opschrijft.
///
/// Kent geen van beide wegen een cel, dan is er **geen** cel — en dat is geen
/// fout. Een terugvordering op een burger is een echte verplichting, ook in een
/// wereld waarin die burger niet meedoet; de termijn wordt dan ingeroosterd en
/// blijft openstaan.
#[derive(Debug, Clone, Default)]
pub(crate) struct PartyBindings {
    /// Uit `komt_na`, op genormaliseerde naam (zie [`normalised`]).
    komt_na: BTreeMap<String, String>,
    /// Uit `identity`, op genormaliseerde naam.
    ///
    /// `None` als meer dan één cel diezelfde naam draagt: dan is niet te zeggen
    /// welke van de twee betaalt, en kiezen zou een betaling bij een willekeurige
    /// organisatie laten landen. `komt_na` kan het alsnog zeggen.
    identities: BTreeMap<String, Option<String>>,
}

impl PartyBindings {
    /// Bind een cel aan de naam die zij met `komt_na` nakomt.
    ///
    /// Geeft de binding terug die er al stond: twee cellen die dezelfde naam
    /// nakomen is een fout in het wereldbestand, en die hoort bij het optuigen te
    /// vallen met beide cellen erbij.
    pub(crate) fn bind(&mut self, name: &str, cell: &str) -> Option<String> {
        self.komt_na.insert(normalised(name), cell.to_string())
    }

    /// Onthoud onder welke naam een cel zich uitgeeft.
    pub(crate) fn note_identity(&mut self, identity: &str, cell: &str) {
        self.identities
            .entry(normalised(identity))
            .and_modify(|found| *found = None)
            .or_insert_with(|| Some(cell.to_string()));
    }

    /// De cel die onder deze naam nakomt, of `None`.
    pub(crate) fn cell_for(&self, name: &str) -> Option<&str> {
        let key = normalised(name);
        if let Some(cell) = self.komt_na.get(&key) {
            return Some(cell);
        }
        self.identities.get(&key)?.as_deref()
    }
}

/// Wat een cel bij een besluit van buiten aangereikt krijgt, omdat ze het zelf
/// niet houdt (RFC-022 §2): wie zij is, hoe laat het is, wat de wereld heeft
/// ingesteld, en wie er betaalt.
///
/// - `identity` is de naam waaronder de cel zich uitgeeft — de bewering uit
///   haar veiligheidscontext, die de wet straks naast haar `competent_authority`
///   legt. Een cel houdt geen veiligheidscontext, dus de naam komt van wie die
///   wél houdt: in deze opstelling [`crate::World`].
/// - `op_moment` is het moment van het besluit — de klok woont in de wereld.
/// - `settings` zijn de instellingen van het wereldbestand; een verplichting met
///   `ritme: $betalingsritme` leest eruit.
/// - `parties` is welke cel er onder welke naam nakomt (`komt_na` en de
///   identiteiten in het wereldbestand). Wát een besluit oplegt en aan wie staat
///   in de wet; wie het nakomt is uitvoering, en een cel kent de andere cellen
///   niet.
///
/// Vier dingen die een cel niet is, in één waarde: dat ze samen aangereikt
/// worden en niet elk apart, is de vorm van die scheiding.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DecisionContext<'a> {
    /// De naam waaronder de besluitende cel zich uitgeeft.
    pub(crate) identity: &'a str,
    /// Het moment waarop besloten wordt.
    pub(crate) op_moment: NaiveDate,
    /// De instellingen van de wereld.
    pub(crate) settings: &'a BTreeMap<String, Value>,
    /// Per partij de cel die er in deze wereld betalingsverplichtingen voor nakomt.
    pub(crate) parties: &'a PartyBindings,
}

/// Wat [`Cell::decide`] heeft uitgezocht voordat de engine draait.
///
/// De volgorde van `decide` is de reden dat dit een waarde is: het zaakkenmerk,
/// het bevoegd gezag en wat het lexogram oplegt staan vast vóór de uitvoering —
/// een besluit dat om zijn kenmerk of om het gezag toch niet genomen kan worden,
/// hoort geen engine te laten draaien en geen andere cel iets te vragen. Ze
/// samen doorgeven houdt die volgorde zichtbaar.
struct DecisionDraft<'a> {
    /// Het zaakkenmerk van dit besluit.
    zaakkenmerk: String,
    /// Wat de besluit-definitie zelf aanleverde, met herkomst.
    inputs: BTreeMap<String, DecretogramInput>,
    /// Het bevoegd gezag dat de regeling aanwijst, al opgelost.
    authority: Option<String>,
    /// Wat het lexogram op dit moment oplegt.
    declared: &'a DeclaredObligations,
}

/// Eén chronolexocel.
///
/// De cel bezit haar feiten. Er is met opzet geen `pub fn store()` en geen
/// publiek veld: dat een andere cel niet bij deze kronieken kan, is een
/// compileerfout en geen afspraak. De enige publieke ingang voor een consument
/// is [`Cell::reduce`].
pub struct Cell {
    /// Het cel-id, alleen voor foutmeldingen en herkomst in het antwoord.
    ///
    /// Wat hier met opzet **niet** staat: wie de cel zegt te zijn. Die bewering
    /// is een eigenschap van haar veiligheidscontext (RFC-022 §2) en komt bij
    /// een besluit van buiten mee — zie [`Self::decide`]. Een cel houdt
    /// kronieken en reduceert; een naam waarmee ze zich uitgeeft, is iets wat
    /// een ondertekening straks moet bewijzen, en dat is niet haar werk.
    id: String,
    /// De regelingen die deze cel laadt, bij `$id`.
    ///
    /// Niet om er iets mee te doen — daarvoor is er een engine — maar omdat het
    /// beeld van de wereld hoort te laten zien welk recht waar geladen is. Een
    /// cel met een lege lijst is een bron-cel, en dat is te zien.
    laws: Vec<String>,
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
    /// [`Self::reduce`] krijgt nooit een [`CellResolver`] en kan de celgrens dus
    /// niet over: een cross-cel-pull vanuit een reductie is daarmee een
    /// ontbrekende capability en geen afspraak (RFC-022 §4.2, tier 3). Deze is
    /// de enige die er een krijgt, en alleen voor de duur van één besluit — zie
    /// [`Self::decide`].
    ///
    /// Alleen aanwezig als de cel besluit-definities heeft: een cel die niet
    /// besluit, heeft aan één engine genoeg.
    besluit_service: Option<RefCell<LawExecutionService>>,
    /// De eigen feiten. Privé, en dat is het punt.
    chronicles: ChronicleStore,
    /// Per kroniekstroom de veldnamen die deze cel van die stroom kent, zoals bij
    /// het optuigen vastgesteld.
    ///
    /// Dezelfde kennis waarmee de definities van de cel getoetst zijn, bewaard
    /// zodat de wereld haar **acties** en **termijnen** er langs dezelfde weg
    /// tegen kan toetsen (zie [`Self::check_stream`]). Zou de wereld een eigen
    /// lijstje bijhouden, dan zou een stroom die de cel niet houdt in een actie
    /// stil goedgekeurd worden.
    streams: config::StreamFields,
    /// De gepubliceerde lexostatussen, op naam.
    published: BTreeMap<String, LexostatusDefinition>,
    /// De besluiten die deze cel kan nemen, op naam.
    besluiten: BTreeMap<String, BesluitDefinition>,
    /// Per besluit het schema van het decretogram dat het kan voortbrengen.
    ///
    /// Bij het optuigen uitgerekend en daarna onveranderlijk, net als de
    /// definities zelf: het hangt aan de wetten die deze cel laadt en aan het
    /// wereldbestand, en die staan vanaf dat moment vast. Een beeld van de wereld
    /// is er daarmee een opzoeking en geen berekening — zie [`schema`].
    besluit_schemas: BTreeMap<String, Vec<DecretogramField>>,
    /// Wat de geladen versies van haar wetten aan verplichtingen opleggen.
    ///
    /// Bewaard om dezelfde reden als [`Self::streams`]: de wereld moet er bij het
    /// optuigen tegenaan kunnen toetsen — is er een cel aan dit gezag gebonden,
    /// houdt die een betalingsstroom — en dat hoort langs dezelfde weg te gaan
    /// als de toetsen van de cel zelf. Bij het besluit wordt de declaratie
    /// opnieuw opgezocht, op de versie die dán geldt.
    obligations: ObligationsPerOutput,
    /// De cel-bronnen van haar wetten (tier 3), op `(cel, uitkomst)`.
    ///
    /// Wat de cel hiermee doet is niets: ze kan geen van deze cellen bereiken.
    /// Het is de afspraak die de wereld nodig heeft om een tier-3-verwijzing bij
    /// de juiste lexostatus van de juiste peer uit te laten komen.
    accepts_from: BTreeMap<(String, String), AcceptedSource>,
    /// Elke naam die de wetten van deze cel in een `source.regulation` noemen en
    /// die geen regeling is die zij zelf laadt.
    ///
    /// Niet om er iets mee te bereiken — dat kan de cel niet — maar om een
    /// engine-melding te kunnen plaatsen. "Regeling niet gevonden" over een naam
    /// die in de eigen wet staat, stuurt de lezer naar een corpusbestand terwijl
    /// er een afspraak mist; zie [`Self::explain_undeclared_source`].
    foreign_sources: BTreeSet<String>,
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
                // Geen schema: wélke velden een decretogram draagt, hangt aan de
                // uitkomsten van de regeling die het besluit uitvoert, en die
                // staan in het lexogram. Wat er vast in staat, staat als schema
                // bij het besluit (zie [`crate::BesluitDefinitionSnapshot`]) —
                // dáár hoort het, want het geldt per besluit en niet per stroom.
                gebeurtenissen: Vec::new(),
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
            // Een hook die op een besluit vuurt waarvoor zijn inputs niet
            // bestaan, laat het besluit niet omvallen: het gram noemt hem onder
            // `hook_niet_uitgevoerd`. Alleen hier, op het besluit-pad — een
            // reductie legt niets vast en heeft dus ook geen gram om het in te
            // zeggen.
            build_service(&laws)?.map(|mut service| {
                service.set_skip_hooks_with_missing_inputs(true);
                service
            })
        };

        let mut declared = chronicles.declared_fields();
        for (stream, fields) in fixture_fields {
            declared
                .entry(stream.clone())
                .or_default()
                .extend(fields.iter().cloned());
        }
        // Wat de hooks op de stage BEKENDMAKING kunnen opleveren, uit de wet.
        let hook_outputs = service.as_ref().map(stage_hook_outputs).unwrap_or_default();
        // Wat de wetten van deze cel onder `chronolex` declareren. Vóór de
        // surface, want elke besluit-definitie wordt eraan getoetst — en over de
        // engine die er is zodra de cel wetten laadt, niet over die van het
        // besluit-pad: een blok dat niet te lezen is, is een fout in de wet, ook
        // in een cel die er (nog) geen besluit op neemt.
        let chronolex = match service.as_ref() {
            None => DeclaredChronolex::empty(),
            Some(service) => chronolex_per_output(service, &hook_outputs)?,
        };
        if !config.besluit_definitions.is_empty() {
            let fields = declared.entry(BESCHIKKINGEN.to_string()).or_default();
            fields.extend(besluit::declared_fields(&config.besluit_definitions));
            // En wat de volgende stage erbij legt. Het gram van een bekendmaking
            // ligt in dezelfde stroom en draagt wat de algemene wet er bij die
            // stage aan hangt, en wat de eigen regeling er voor die stage
            // declareert; een reductie mag daar dus over publiceren.
            fields.extend(hook_outputs.iter().cloned());
            fields.extend(
                chronolex
                    .stage_uitkomsten
                    .values()
                    .flat_map(BTreeMap::values)
                    .flatten()
                    .cloned(),
            );
        }
        let obligations = chronolex.obligations;
        let stage_uitkomsten = chronolex.stage_uitkomsten;

        let surface = CellSurface {
            laws: &config.laws,
            obligations: obligations.clone(),
            outputs: service
                .as_ref()
                .map(outputs_per_regulation)
                .unwrap_or_default(),
            legal_characters: service
                .as_ref()
                .map(legal_characters_per_output)
                .unwrap_or_default(),
            output_types: service.as_ref().map(types_per_output).unwrap_or_default(),
            afwijzing_blocks: chronolex.afwijzing_blocks,
            regulation_inputs: service
                .as_ref()
                .map(inputs_per_regulation)
                .unwrap_or_default(),
            stream_keys: chronicles.declared_keys(),
            streams: declared,
            besluit_fields: config
                .besluit_definitions
                .iter()
                .map(|definition| (definition.name.clone(), definition.gram_fields()))
                .collect(),
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
            check_stage_uitkomsten(&config.id, definition, &stage_uitkomsten, &hook_outputs)?;
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

        // Ná het valideren: een schema van een definitie die geweigerd wordt, is
        // een beeld van iets dat niet bestaat.
        let besluit_schemas = besluiten
            .values()
            .map(|definition| {
                (
                    definition.name.clone(),
                    schema::decretogram_schema(definition, service.as_ref()),
                )
            })
            .collect();

        let CellSources {
            declared: accepts_from,
            foreign: foreign_sources,
        } = check_accepted_sources(config, service.as_ref())?;

        Ok(Self {
            id: config.id.clone(),
            laws: config.laws.clone(),
            service: service.map(RefCell::new),
            besluit_service: besluit_service.map(RefCell::new),
            chronicles,
            streams: surface.streams,
            published,
            besluiten,
            besluit_schemas,
            obligations,
            accepts_from,
            foreign_sources,
        })
    }

    /// Het cel-id.
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// De regelingen die deze cel laadt, bij `$id`.
    pub(crate) fn laws(&self) -> &[String] {
        &self.laws
    }

    /// De namen die deze cel publiceert, in alfabetische volgorde.
    pub(crate) fn published_names(&self) -> Vec<&str> {
        self.published.keys().map(String::as_str).collect()
    }

    /// De gepubliceerde lexostatussen zoals de cel ze documenteert, in
    /// alfabetische volgorde.
    ///
    /// De definitie en niet alleen haar naam: een consument die vraagt, hoort te
    /// kunnen lezen waar de naam over gaat en welke parameters ze verlangt. Die
    /// belofte staat hier al vast sinds het optuigen (zie
    /// [`LexostatusDefinition::validate`]); wie haar alleen in het wereldbestand
    /// laat staan, laat een vrager raden wat hij moet meegeven.
    pub(crate) fn published_definitions(&self) -> impl Iterator<Item = &LexostatusDefinition> {
        self.published.values()
    }

    /// De besluit-definities van deze cel, in alfabetische volgorde.
    pub(crate) fn besluit_definitions(&self) -> impl Iterator<Item = &BesluitDefinition> {
        self.besluiten.values()
    }

    /// Het schema van het decretogram dat dit besluit kan voortbrengen, zoals bij
    /// het optuigen uitgerekend; leeg als de cel dit besluit niet kent.
    pub(crate) fn besluit_schema(&self, besluit: &str) -> &[DecretogramField] {
        self.besluit_schemas.get(besluit).map_or(&[], Vec::as_slice)
    }

    /// Houdt deze cel deze stroom?
    ///
    /// Dezelfde weigering als bij een stroom in een definitie, zonder een veld:
    /// een termijn wijst een stroom en een gram-naam aan, en een naam is geen
    /// veld.
    pub(crate) fn check_stream(&self, subject: Subject, name: &str, stream: &str) -> Result<()> {
        config::fields_of_stream(&self.streams, &self.id, subject, name, stream).map(|_| ())
    }

    /// De laatste waarde die één veld in één kroniek van deze cel kreeg.
    ///
    /// Waarmee de wereld een formulierveld kan voorvullen met wat zij al weet
    /// (zie [`crate::cell::Prefill`]). Dezelfde weg als [`Self::inspect`] en om
    /// dezelfde reden `pub(crate)`: de wereld bezit de cellen en maakt het beeld,
    /// en een cel die dit kon aanroepen zou de kroniek van een ander lezen.
    ///
    /// `None` als er nog niets ligt. Dat is een antwoord en geen fout: een
    /// formulier dat niet voorgevuld kan worden, staat leeg.
    pub(crate) fn last_value(
        &self,
        stream: &str,
        field: &str,
        op_moment: NaiveDate,
    ) -> Option<&Value> {
        self.chronicles.last_value(stream, field, op_moment)
    }

    /// Ligt er op of vóór `op_moment` een gram met deze naam in deze stroom?
    ///
    /// Voor een termijn die waarschuwt als een feit ontbreekt: dat is een vraag
    /// over de naam van het gram, niet over een veldwaarde.
    pub(crate) fn has_recording_named(
        &self,
        stream: &str,
        name: &str,
        op_moment: NaiveDate,
    ) -> bool {
        self.chronicles.contains_named(stream, name, op_moment)
    }

    /// Kan deze cel op eigen naam een feit met deze velden in deze stroom leggen?
    ///
    /// De toets van [`Self::record`], vooruitgeschoven naar het optuigen: een
    /// actie noemt haar formulier en haar stroom, en dan staat hier al vast of het
    /// gram dat eruit komt ooit kan landen. Een actie die pas bij de eerste klik
    /// omvalt, is een typfout die op het verkeerde moment boven water komt.
    ///
    /// Geeft de reden als tekst en niet als [`SimulatorError`]: *waarom* het niet
    /// kan weet de cel, *welke actie* het was weet de wereld, en die twee horen in
    /// één melding samen te komen (zie [`SimulatorError::ActionRecording`]).
    pub(crate) fn check_recordable(
        &self,
        stream: &str,
        name: &str,
        form: &[DocumentedParameter],
    ) -> std::result::Result<(), String> {
        if stream == BESCHIKKINGEN {
            return Err(format!(
                "'{BESCHIKKINGEN}' is voorbehouden aan het besluit-pad; daar ontstaat een \
                 gram door te besluiten en niet door het vast te leggen"
            ));
        }
        let Some(key) = self.chronicles.key_of(stream) else {
            return Err(format!(
                "die cel houdt geen stroom met die naam (wel: {})",
                self.chronicles.stream_names().join(", ")
            ));
        };
        if !form
            .iter()
            .any(|field| field.name.eq_ignore_ascii_case(key))
        {
            return Err(format!(
                "die stroom groepeert op veld '{key}', en dat veld staat niet in het \
                 formulier (wel: {})",
                config::parameter_listing(form)
            ));
        }
        // En het schema van de stroom, als ze er een declareert: wat een actie
        // vastlegt staat bij het optuigen al vast, dus een gram dat straks niet
        // door de toets zou komen hoort hier te stranden en niet bij de eerste
        // druk op de knop.
        self.chronicles.check_form(stream, name, form)
    }

    /// Het gebeurtenisschema van één stroom van deze cel; leeg als ze er geen
    /// declareert.
    ///
    /// Voor de wereld, die toetst of de stroom waarin straks betaald wordt het
    /// ingebouwde schema van een betaling bijhoudt — en voor het beeld, dat het
    /// schema bij de stroom toont.
    pub(crate) fn stream_schema(&self, stream: &str) -> &[GebeurtenisSchema] {
        self.chronicles.schema_of(stream)
    }

    /// De kronieken zoals ze erbij liggen, geleend voor het beeld van de wereld.
    ///
    /// Dit is de enige weg naar de inhoud van een kroniek buiten een reductie om,
    /// en hij bestaat voor precies één doel: de wereld toont wat waar ligt (zie
    /// [`crate::Snapshot`]). Dat is een inspectiebeeld, zoals het observatielog
    /// een meetinstrument is — en het is met opzet `pub(crate)`, want een cel die
    /// het kon aanroepen zou de kroniek van een ander lezen, en dan is invariant
    /// I1 een afspraak in plaats van een compileerfout.
    pub(crate) fn inspect(&self) -> Vec<ChronicleView<'_>> {
        self.chronicles.view()
    }

    /// Hoeveel grammen er in één kroniekstroom liggen; `None` als de cel haar
    /// niet houdt.
    ///
    /// Uit hetzelfde inspectiebeeld als [`Self::inspect`], en om dezelfde reden
    /// `pub(crate)`: de wereld wijst met de plek in de kroniek naar een gram dat
    /// het beeld al geeft (zie [`crate::journal::GramRef`]), en een cel komt er
    /// niet aan.
    pub(crate) fn stream_len(&self, stream: &str) -> Option<usize> {
        Some(self.stream_view(stream)?.len())
    }

    /// De plek en het gram van de laatste vastlegging in één stroom.
    ///
    /// `None` als de stroom leeg is of niet bestaat. Zie [`Self::stream_len`]
    /// voor waarom dit het inspectiebeeld is en geen tweede ingang.
    pub(crate) fn last_gram(&self, stream: &str) -> Option<(usize, &ChronicleEvent)> {
        let events = self.stream_view(stream)?;
        let index = events.len().checked_sub(1)?;
        Some((index, events.get(index)?))
    }

    /// Eén gram uit één kroniekstroom, op zijn plek in de volgorde van
    /// vastlegging.
    ///
    /// `None` als de cel de stroom niet houdt of er op die plek niets ligt. Uit
    /// hetzelfde inspectiebeeld als [`Self::inspect`] en om dezelfde reden
    /// `pub(crate)`: de wereld mag een gram aanwijzen dat het beeld al geeft
    /// (bijvoorbeeld om er het uitvoeringsreceipt van te lezen), een cel komt er
    /// niet aan.
    pub(crate) fn gram(&self, stream: &str, index: usize) -> Option<&ChronicleEvent> {
        self.stream_view(stream)?.get(index)
    }

    /// De kroniekstromen die deze cel houdt, in de volgorde van haar
    /// configuratie. Voor de melding die zegt welke er wél zijn.
    pub(crate) fn stream_names(&self) -> Vec<&str> {
        self.chronicles
            .view()
            .into_iter()
            .map(|view| view.stream)
            .collect()
    }

    /// De grammen van één stroom, geleend uit het inspectiebeeld.
    fn stream_view(&self, stream: &str) -> Option<&[ChronicleEvent]> {
        self.chronicles
            .view()
            .into_iter()
            .find(|view| view.stream == stream)
            .map(|view| view.events)
    }

    /// De cel-bronnen die de wetten van deze cel aanwijzen (tier 3).
    ///
    /// `pub(crate)`: dit is geen weg naar een andere cel — de cel heeft er geen
    /// — maar de afspraak die de wereld uitvoert als de engine tijdens een
    /// besluit om zo'n waarde vraagt.
    pub(crate) fn accepts_from(&self) -> impl Iterator<Item = &AcceptedSource> {
        self.accepts_from.values()
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

        let (outcome, reductie) = match &definition.reduction {
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
                aggregate,
            } => {
                let query = ChronicleQuery {
                    chronicle,
                    key,
                    key_value: self.key_value(definition, key, params)?,
                    conditions,
                };
                match aggregate {
                    Aggregate::Latest => self.filter_chronicle(definition, &query, op_moment)?,
                    Aggregate::Sum { field } => {
                        self.sum_chronicle(definition, &query, field, op_moment)?
                    }
                }
            }
            // De openstaandvorm kent geen `where` en geen aggregatie: wat ze met
            // de grammen doet, ligt in de vorm vast. Ze krijgt daarom de sleutel
            // en niet een [`ChronicleQuery`] — dat type draagt precies de twee
            // dingen die hier niet bestaan.
            Reduction::Openstaand { key } => openstaand::reduce(
                self,
                key,
                self.key_value(definition, key, params)?,
                op_moment,
            )?,
        };

        Ok(Lexostatus {
            cell: self.id.clone(),
            name: definition.name.clone(),
            op_moment,
            outcome,
            reductie: Some(reductie),
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
    ) -> Result<(LexostatusOutcome, Reductie)> {
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

        let engine_params = engine_parameters(parameters, params);
        let result = service
            .evaluate_law_output(
                regulation,
                output,
                engine_params.clone(),
                &op_moment.format("%Y-%m-%d").to_string(),
            )
            .map_err(|error| self.explain_reach(definition, error))?;

        let (inputs, grammen) = self.explain_inputs(&result, parameters, &engine_params, op_moment);
        let wetsvorm = Wetsvorm {
            regulation: regulation.to_string(),
            regulation_valid_from: result.regulation_valid_from.clone(),
            output: output.to_string(),
            inputs,
            op_moment,
        };

        Ok((
            LexostatusOutcome::Established(definition.project(result.outputs)),
            Reductie::vastgesteld(wetsvorm, grammen),
        ))
    }

    /// Waar elke input van deze uitvoering vandaan kwam, en welke grammen daar
    /// achter zaten.
    ///
    /// De engine meldt per input welke tier van de resolutievolgorde hem
    /// beantwoordde (RFC-022 §4.2); wat zij niet kan melden, is welk *gram* van
    /// een kroniekstroom dat geworden is — zij ziet per onderwerp één record,
    /// want de tijdreductie is er dan al overheen gegaan. Dat laatste weet
    /// alleen de cel, en daarom staat het hier.
    ///
    /// De inputs van déze uitvoering, en niet van de regelingen die zij op haar
    /// beurt aanriep: elke aanroep heeft haar eigen herkomst, en die uitvouwen
    /// zou van deze uitleg een trace maken.
    fn explain_inputs(
        &self,
        result: &ArticleResult,
        bindings: &BTreeMap<String, String>,
        engine_params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> (Vec<GebruikteInput>, Vec<GebruiktGram>) {
        let mut grammen: Vec<GebruiktGram> = Vec::new();
        let mut inputs: Vec<GebruikteInput> = Vec::new();

        for (name, provenance) in &result.input_provenance {
            let herkomst = match provenance {
                InputProvenance::Parameter => InputHerkomst::Parameter {
                    // De naam waaronder de *vraag* hem kende, en niet de naam
                    // die de regeling hem geeft: een consument herkent zijn
                    // eigen parameter. Een binding zonder `$` is een vaste
                    // waarde uit de definitie en dus geen parameter.
                    parameter: bindings
                        .get(name)
                        .and_then(|binding| binding_name(binding))
                        .map(str::to_string),
                },
                InputProvenance::DataSource { source } => {
                    let gram =
                        self.gram_behind(source, engine_params, op_moment)
                            .map(|(place, event)| {
                                let gebruikt = GebruiktGram::new(&self.id, source, place, event);
                                let id = gebruikt.gram.id.clone();
                                if !grammen.iter().any(|known| known.gram.id == id) {
                                    grammen.push(gebruikt);
                                }
                                id
                            });
                    InputHerkomst::EigenKroniek {
                        chronicle: source.clone(),
                        gram,
                    }
                }
                InputProvenance::Regulation { regulation, output } => InputHerkomst::Regeling {
                    regulation: regulation.clone(),
                    output: output.clone(),
                },
                InputProvenance::Cell { cell, output } => InputHerkomst::Cel {
                    cell: cell.clone(),
                    output: output.clone(),
                },
            };

            inputs.push(GebruikteInput {
                name: name.clone(),
                herkomst,
            });
        }

        // In de volgorde waarin ze in de kronieken liggen, en niet in die waarin
        // de inputs erom vroegen: dit is een lijst grammen, en die hoort te
        // lezen als een kroniek.
        grammen.sort_by(|left, right| {
            left.gram
                .chronicle
                .cmp(&right.gram.chronicle)
                .then(left.volgnummer.cmp(&right.volgnummer))
        });
        (inputs, grammen)
    }

    /// Welk gram van deze stroom stond bij deze uitvoering voor dit onderwerp
    /// klaar?
    ///
    /// Dezelfde vraag als het kroniekfilter stelt, en met opzet langs dezelfde
    /// weg: de stroom staat als databron klaar met per sleutelwaarde de laatste
    /// vastlegging op of vóór dit moment (zie [`Self::register_own_facts`]), en
    /// de engine zoekt daarin op de sleutel die in haar parameters staat. Wie
    /// dat hier anders zou uitrekenen, zou een ander gram kunnen noemen dan
    /// waarop gerekend is.
    ///
    /// `None` als de cel de stroom niet houdt, als de vraag haar sleutel niet
    /// meegaf, of als er over dit onderwerp niets lag: dan is er geen gram om
    /// naar te wijzen, en de herkomst noemt alleen de stroom.
    fn gram_behind(
        &self,
        stream: &str,
        engine_params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Option<(usize, &ChronicleEvent)> {
        let key = self.chronicles.key_of(stream)?;
        // Hoofdletterongevoelig, zoals de engine haar databronnen bevraagt.
        let key_value = engine_params
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value)?;
        let no_conditions = BTreeMap::new();
        self.chronicles.latest_recording_with_place(
            stream,
            key,
            key_value,
            &no_conditions,
            op_moment,
        )
    }

    /// Vertaal "onbekende regeling" naar "die naam is een cel" waar dat zo is.
    ///
    /// De reduce-engine heeft geen cel-tier, dus een `source.regulation` die een
    /// cel aanwijst is voor haar een regeling die ze niet kent — en dat is
    /// letterlijk waar, maar het verbergt wat er gebeurde. De cel weet wél dat
    /// die naam een peer is (haar configuratie zegt het), dus ze kan de melding
    /// geven die de lezer nodig heeft: hier reikt een reductie buiten haar cel,
    /// en dat kan niet.
    fn explain_reach(
        &self,
        definition: &LexostatusDefinition,
        error: regelrecht_engine::EngineError,
    ) -> SimulatorError {
        if let regelrecht_engine::EngineError::LawNotFound(name) = &error {
            if self.accepts_from.keys().any(|(cell, _)| cell == name) {
                return SimulatorError::ReductionReachesOutsideCell {
                    cell: self.id.clone(),
                    lexostatus: definition.name.clone(),
                    peer: name.clone(),
                };
            }
        }
        error.into()
    }

    /// Vertaal "onbekende regeling" naar "die naam staat in je eigen wet" waar dat
    /// zo is.
    ///
    /// Het tegenhanger van [`Self::explain_reach`], voor het besluit-pad. Daar
    /// gaat het om een cel die de cel-tier wél bereikt maar de reductie niet; hier
    /// om een naam die de cel-tier helemaal niet bereikt, omdat `accepts_from`
    /// haar niet declareert. De engine noemt dat een ontbrekende regeling, wat de
    /// lezer een corpusbestand laat zoeken dat er niet hoort te zijn.
    fn explain_undeclared_source(
        &self,
        definition: &BesluitDefinition,
        error: regelrecht_engine::EngineError,
    ) -> SimulatorError {
        if let regelrecht_engine::EngineError::LawNotFound(name) = &error {
            let declared = self.accepts_from.keys().any(|(cell, _)| cell == name);
            if !declared && self.foreign_sources.contains(name) {
                return SimulatorError::UndeclaredCellSource {
                    cell: self.id.clone(),
                    besluit: definition.name.clone(),
                    name: name.clone(),
                };
            }
        }
        error.into()
    }

    /// Zet de eigen feiten zoals ze op dit moment waren klaar als databron.
    ///
    /// De stroom met decretogrammen blijft er met opzet buiten. Een besluit is
    /// geen feit om op te rekenen maar een uitkomst om terug te lezen; zou ze
    /// als databron meedoen, dan zou een volgende uitvoering stil op de eigen
    /// uitkomst van een eerder besluit kunnen leunen, en dan weet niemand meer
    /// of er gerekend of overgeschreven is. Terugzien doe je met een reductie
    /// over die stroom (`Cell::reduce`), en die komt niet langs de engine.
    ///
    /// Wat hier klaargezet wordt, legt het besluit-pad met stand en hash in
    /// zijn gram (zie [`Decretogram::chronicle_sources`], RFC-022 §1.3); een
    /// reductie legt niets vast en laat dat achterwege.
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

    /// Geef de besluit-engine de cel-tier, voor de duur van dit ene besluit.
    ///
    /// De resolver komt van buiten en wordt hier alleen doorgegeven: de cel
    /// bouwt hem niet en kan hem niet bevragen. Wat ze wél bepaalt, is *voor
    /// welke cel-ids* hij bereikbaar is — precies de bronnen die haar eigen
    /// wetten noemen en die haar configuratie heeft uitgewerkt. Een cel-id
    /// daarbuiten blijft voor de engine een onbekende regeling en dus een fout,
    /// en dat is invariant I3 als capability in plaats van als afspraak.
    ///
    /// Zonder gedeclareerde bronnen gebeurt er niets: dan is er geen cel-tier en
    /// gedraagt de besluit-engine zich als de reduce-engine.
    fn grant_cell_tier(
        &self,
        service: &mut LawExecutionService,
        resolver: Option<Rc<dyn CellResolver>>,
    ) -> Result<()> {
        let Some(resolver) = resolver else {
            return Ok(());
        };
        let ids: BTreeSet<&str> = self
            .accepts_from
            .values()
            .map(|source| source.cell.as_str())
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        service.set_cell_resolver(ids, resolver)?;
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
        query: &ChronicleQuery<'_>,
        op_moment: NaiveDate,
    ) -> Result<(LexostatusOutcome, Reductie)> {
        let filter = query.describe(Regel::Laatste, op_moment);
        let Some((place, event)) = self.chronicles.latest_recording_with_place(
            query.chronicle,
            query.key,
            query.key_value,
            query.conditions,
            op_moment,
        ) else {
            return Ok((
                LexostatusOutcome::NotEstablished {
                    reason: query.nothing_established(op_moment),
                },
                Reductie::niets_vastgesteld(filter, Vec::new(), query.missed(self, op_moment)),
            ));
        };
        let gelezen = vec![GebruiktGram::new(&self.id, query.chronicle, place, event)];

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
            return Ok((
                LexostatusOutcome::NotEstablished {
                    reason: nothing_published(definition, query, event.op_moment),
                },
                // Mét het gram: er is er wél een gelezen, en dat de uitkomsten
                // er niet in stonden, is aan dat gram te zien en nergens anders.
                Reductie::niets_vastgesteld(filter, gelezen, query.missed(self, op_moment)),
            ));
        }

        Ok((
            LexostatusOutcome::Established(values),
            Reductie::vastgesteld(filter, gelezen),
        ))
    }

    /// De som over een eigen kroniek: tel één veld op over alles wat op dit
    /// moment vastlag.
    ///
    /// De eerste aggregatie van de simulator. Ze bestaat zodat een totaal — wat
    /// er tot nu toe betaald is — een **reductie over vastleggingen** kan zijn in
    /// plaats van een saldo dat ergens bijgehouden wordt. Een saldo zou een
    /// tweede waarheid naast de kroniek zijn, en dan verandert het beeld van een
    /// eerder moment zodra er iets bijkomt.
    ///
    /// Een vastlegging die het veld niet draagt of er iets anders dan een getal
    /// in heeft, is een fout en geen nul: een som die zo'n vastlegging overslaat
    /// valt stil te laag uit.
    fn sum_chronicle(
        &self,
        definition: &LexostatusDefinition,
        query: &ChronicleQuery<'_>,
        field: &str,
        op_moment: NaiveDate,
    ) -> Result<(LexostatusOutcome, Reductie)> {
        let filter = query.describe(
            Regel::Som {
                field: field.to_string(),
            },
            op_moment,
        );
        let events = self.chronicles.recordings_with_place(
            query.chronicle,
            query.key,
            query.key_value,
            query.conditions,
            op_moment,
        );

        // Nul vastleggingen is niet "nul euro": deze cel heeft over dit
        // onderwerp niets vastgelegd, en dat is hetzelfde antwoord als bij elk
        // ander kroniekfilter. Een 0 zou "er is niets betaald" niet kunnen
        // onderscheiden van "hier is geen zaak".
        if events.is_empty() {
            return Ok((
                LexostatusOutcome::NotEstablished {
                    reason: query.nothing_established(op_moment),
                },
                Reductie::niets_vastgesteld(filter, Vec::new(), query.missed(self, op_moment)),
            ));
        }

        let mut gelezen: Vec<GebruiktGram> = Vec::new();
        let mut total = Decimal::ZERO;
        for (place, event) in events {
            let found = chronicle::field(&event.fields, field);
            let value = found.and_then(Value::as_decimal).ok_or_else(|| {
                SimulatorError::SumOfNonNumber {
                    cell: self.id.clone(),
                    lexostatus: definition.name.clone(),
                    field: format!("{}.{field}", query.chronicle),
                    op_moment: event.op_moment.to_string(),
                    found: found.map_or_else(
                        || "dat veld niet".to_string(),
                        |value| format!("{} ({})", value, value.type_name()),
                    ),
                }
            })?;
            total += value;
            // Elk meegeteld gram met wat het bijdroeg: een som waarvan alleen de
            // uitkomst te zien is, valt niet na te rekenen, en dan is het
            // verschil met een saldo dat ergens bijgehouden wordt alleen nog een
            // belofte.
            gelezen.push(
                GebruiktGram::new(&self.id, query.chronicle, place, event)
                    .met_bijdrage(amount(value)),
            );
        }

        // Onder de gepubliceerde naam en niet onder de naam in `sum`. Het filter
        // hiernaast doet dat ook, en die twee mogen niet uiteenlopen: het optuigen
        // vergelijkt `outputs` met het gesommeerde veld zonder op kapitalen te
        // letten, dus de twee schrijfwijzen kunnen verschillen — en dan zou het
        // antwoord onder een naam staan die de definitie niet publiceert.
        // Onbereikbaar leeg: `validate_chronicle` eist precies één uitkomst.
        let published = definition
            .published_outputs()
            .into_iter()
            .next()
            .unwrap_or(field);

        Ok((
            LexostatusOutcome::Established(BTreeMap::from([(
                published.to_string(),
                amount(total),
            )])),
            Reductie::vastgesteld(filter, gelezen),
        ))
    }

    /// De waarde van het sleutelveld uit de vraag.
    ///
    /// Onbereikbaar leeg: `validate` eist dat de sleutel een gedocumenteerde
    /// parameter is, en `check_params` dat elke gedocumenteerde parameter
    /// meekomt. Eén plek, want beide kroniekfilters stellen dezelfde vraag.
    fn key_value<'a>(
        &self,
        definition: &LexostatusDefinition,
        key: &str,
        params: &'a BTreeMap<String, Value>,
    ) -> Result<&'a Value> {
        params
            .get(key)
            .ok_or_else(|| SimulatorError::MissingParameter {
                cell: self.id.clone(),
                subject: Subject::Lexostatus,
                name: definition.name.clone(),
                parameter: key.to_string(),
            })
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
    /// 4. de verplichtingen worden uitgerekend tot een schema van termijnen, op
    ///    de uitkomsten waarop besloten is en met de instellingen van de wereld;
    /// 5. de uitkomst wordt als decretogram vastgelegd in de eigen stroom
    ///    [`BESCHIKKINGEN`] — één gram, met alle uitkomsten en het hele schema
    ///    samen (RFC-022 §1.2).
    ///
    /// `pub(crate)` en niet `pub`, net als [`Self::record`]: een consument kan een
    /// cel niet laten besluiten. Dat doet de cel zelf, in deze opstelling
    /// aangestuurd door [`crate::World`] op een moment dat de klok heeft bereikt.
    ///
    /// Een input die op dit moment niet op te halen is, is een fout en geen
    /// "niets vastgesteld": een besluit dat een feit mist, hoort niet met een gat
    /// verder te rekenen en al helemaal niet vast te leggen.
    ///
    /// `accepted` zijn de waarden die de cel van een andere cel *accepteert*:
    /// zij vraagt ze niet zelf op (ze houdt geen veiligheidscontext en geen
    /// transport), ze krijgt ze aangereikt voor precies de inputs die haar
    /// definitie met `accept_from` aanwijst — zie [`Self::acceptance_requests`].
    /// `resolver` is dezelfde weg, maar voor de verwijzingen die *de wet* legt
    /// (tier 3): de besluit-engine krijgt hem voor de duur van dit ene besluit,
    /// en de reduce-engine nooit.
    ///
    /// `context` is wat de cel zelf niet houdt en dus aangereikt krijgt: wie zij
    /// is, hoe laat het is en wat de wereld heeft ingesteld — zie
    /// [`DecisionContext`]. De naam waaronder ze zich uitgeeft, toetst ze wél
    /// zelf: tegen het bevoegd gezag dat de wet aanwijst, en dat is haar eigen
    /// weigering, langs welke weg ze ook aangestuurd wordt.
    pub(crate) fn decide(
        &mut self,
        besluit: &str,
        params: &BTreeMap<String, Value>,
        context: DecisionContext<'_>,
        accepted: &BTreeMap<String, DecretogramInput>,
        resolver: Option<Rc<dyn CellResolver>>,
    ) -> Result<Decretogram> {
        let DecisionContext {
            identity,
            op_moment,
            settings,
            parties,
        } = context;
        let definition = self.definition(besluit)?;

        definition.check_params(&self.id, params)?;
        // Vóór het ophalen en het rekenen: een zaak waarvan het kenmerk niet
        // eenduidig is, hoort er helemaal niet te komen — en dan hoeft de engine
        // er ook niet voor te draaien.
        let zaakkenmerk = definition.zaakkenmerk(&self.id, params)?;
        // En vóór het rekenen: wie niet het bevoegd gezag is, neemt geen besluit.
        // Dat dit hier staat en niet alleen bij de aanroeper, is het punt — een
        // cel weigert dit zelf, langs welke weg ze ook aangestuurd wordt.
        let authority = self.competent_authority(&definition, op_moment)?;
        self.check_competent_authority(&definition, identity, authority.as_deref())?;

        // Wát dit besluit oplegt, staat in het lexogram en hangt niet aan de
        // uitkomst: het is hier al bekend, en dat moet ook — de bedragen waarover
        // het gaat, moeten mee de uitvoering in als gevraagde uitkomst.
        let declared = self.declared_obligations(&definition, op_moment)?;
        let inputs = self.collect_inputs(&definition, params, accepted, &zaakkenmerk, op_moment)?;
        let draft = DecisionDraft {
            zaakkenmerk,
            inputs,
            authority,
            declared: &declared,
        };
        let (mut decretogram, computed) = self.execute(&definition, context, draft, resolver)?;
        // Ná de uitvoering, want het bedrag komt uit wat er zojuist uitgerekend
        // is; vóór het vastleggen, want het schema hoort ín het gram. Wie de
        // verplichting nakomt, komt uit de wereld.
        //
        // Een afwijzing legt niets op. Niet "een schema met bedrag nul", en niet
        // "een schema dat we daarna weggooien": een weigering belooft niets, dus
        // er valt niets uit te rekenen. Een besluit dat afwijst op de uitkomst
        // waaruit het bedrag zou komen, zou hier anders omvallen op een bedrag
        // dat de wet terecht niet gegeven heeft.
        if !declared.is_empty() && !decretogram.is_afwijzing() {
            let output_articles = self.output_articles(
                &definition.regulation,
                declared.schedule_outputs(&definition),
                op_moment,
            );
            let schema = definition.schedule_obligations(
                ObligationScope {
                    cell: &self.id,
                    parties,
                    zaakkenmerk: &decretogram.zaakkenmerk,
                    op_moment,
                    output_articles: &output_articles,
                },
                &declared,
                &computed,
                params,
                settings,
            )?;
            decretogram.obligations = schema.termijnen;
            decretogram.wacht_op_bekendmaking = schema.wachtend;
            decretogram.niets_te_betalen = schema.niets_te_betalen;
        }

        let event = decretogram.event()?;
        self.record_own(BESCHIKKINGEN, event)?;
        // Nú pas: de termijnen wijzen naar het gram waarin ze staan, en dat gram
        // heeft zijn plek pas zodra het ligt. Een verwachte plek zou een
        // verwijzing zijn die klopt zolang er niets tussenkomt, en dat is geen
        // verwijzing.
        //
        // Onbereikbaar dat de stroom hierna leeg is: er is net iets in vastgelegd.
        let plek = self.last_gram(BESCHIKKINGEN).map_or(0, |(plek, _)| plek);
        for due in &mut decretogram.obligations {
            due.besluit_gram = plek;
        }

        Ok(decretogram)
    }

    /// Hoe het met de bekendmaking van dit besluit staat, uit de eigen kroniek.
    ///
    /// **Afgeleid, niet gedeclareerd.** Of er bekendgemaakt kan worden, is geen
    /// voorwaarde die iemand in het wereldbestand opschrijft maar een stand van
    /// zaken die in [`BESCHIKKINGEN`] te lezen is: ligt er een gram van de stage
    /// [`STAGE_BESLUIT`] waarvoor nog geen gram van de stage
    /// [`STAGE_BEKENDMAKING`] bestaat? Een droogloop over de eigen kronieken,
    /// dus — er komt geen cel aan de kroniek van een ander en er gaat niets over
    /// een celgrens (invariant I1).
    ///
    /// Het **laatste** besluit-gram en niet het eerste: over één zaak worden
    /// meer besluiten genomen, en wat er bekendgemaakt wordt is het besluit dat
    /// er nu ligt. De volgorde is die van de kroniek, zoals overal in deze
    /// opstelling — twee besluiten op dezelfde dag zijn op de tijdas niet uit
    /// elkaar te houden, dus beslist de volgorde van vastleggen.
    pub(crate) fn bekendmaking_stand(&self, besluit: &str) -> BekendmakingStand {
        let grams = self.stream_view(BESCHIKKINGEN).unwrap_or_default();
        let Some((plek, laatste)) = grams.iter().enumerate().rfind(|(_, event)| {
            gram_tekst(event, besluit::BESLUIT) == besluit
                && gram_tekst(event, besluit::STAGE) == besluit::STAGE_BESLUIT
        }) else {
            return BekendmakingStand::GeenBesluit;
        };
        let zaakkenmerk = gram_tekst(laatste, besluit::ZAAKKENMERK).to_string();
        // Naar dít gram en niet naar deze zaak: over één zaak worden meer
        // besluiten genomen, en dat een eerder besluit bekendgemaakt is, zegt
        // niets over het besluit dat er nu ligt. De verwijzing staat in het gram
        // van de bekendmaking zelf ([`besluit::BESLUIT_GRAM`]) — op het
        // zaakkenmerk zoeken zou het tweede besluit voorgoed onbekendgemaakt
        // laten.
        let naar_dit_gram = Value::Int(i64::try_from(plek).unwrap_or(i64::MAX));
        let bekend = grams.iter().find(|event| {
            gram_tekst(event, besluit::BESLUIT) == besluit
                && gram_tekst(event, besluit::STAGE) == besluit::STAGE_BEKENDMAKING
                && event.fields.get(besluit::BESLUIT_GRAM) == Some(&naar_dit_gram)
        });
        match bekend {
            Some(event) => BekendmakingStand::AlBekendgemaakt {
                zaakkenmerk,
                besluit_op_moment: laatste.op_moment,
                op_moment: event.op_moment,
            },
            None => BekendmakingStand::TeDoen {
                zaakkenmerk,
                plek,
                besluit_op_moment: laatste.op_moment,
            },
        }
    }

    /// Is er over dezelfde zaak ná dit besluit-gram een besluit genomen dat in
    /// de plaats van dit besluit kwam?
    ///
    /// Dezelfde droogloop over de eigen kroniek als [`Self::bekendmaking_stand`]
    /// en om dezelfde reden afgeleid in plaats van gedeclareerd: welk besluit er
    /// over een zaak nog meer ligt, staat in [`BESCHIKKINGEN`] en niet in het
    /// wereldbestand. Er gaat niets over een celgrens (invariant I1) — het
    /// besluit dat vervangt is een besluit van deze cel, want alleen zij beslist
    /// over haar eigen zaken.
    ///
    /// **Ná dit gram en niet vanaf de zaak**: dat er ooit over deze zaak een
    /// vervangende beschikking genomen is, zegt niets over het besluit dat hier
    /// bekendgemaakt wordt — een besluit dat ná die vervanging genomen is, staat
    /// er zelf gewoon. Het eerste dat volgt telt: wat daarna nog kwam, vervangt
    /// niet meer wat al niet meer stond.
    ///
    /// Of een besluit vervangt, staat in het lexogram van het artikel dat het
    /// voortbrengt, op het moment van dát besluit (zie [`Self::vervanging`]).
    fn vervangen_na(&self, plek: usize, zaakkenmerk: &str) -> Result<Option<TermijnenVervallen>> {
        let grams = self.stream_view(BESCHIKKINGEN).unwrap_or_default();
        for event in grams.iter().skip(plek + 1) {
            if gram_tekst(event, besluit::STAGE) != besluit::STAGE_BESLUIT
                || gram_tekst(event, besluit::ZAAKKENMERK) != zaakkenmerk
            {
                continue;
            }
            let naam = gram_tekst(event, besluit::BESLUIT);
            let Some(vervanging) = self.vervanging(naam, event.op_moment)? else {
                continue;
            };
            return Ok(Some(TermijnenVervallen {
                besluit: naam.to_string(),
                op_moment: event.op_moment,
                grondslag: vervanging.grondslag,
            }));
        }
        Ok(None)
    }

    /// De stage BEKENDMAKING zoals de procedure van dit besluit haar declareert:
    /// de procedure en het formulier.
    ///
    /// De procedure komt uit de algemene wet en niet uit het wereldbestand: het
    /// artikel dat de aansturende uitkomst voortbrengt noemt zijn rechtskarakter
    /// en eventueel een `procedure_id`, en de engine zoekt de procedure daarbij
    /// (RFC-008). Het **formulier** van de bekendmaking is wat die stage in
    /// `requires` vraagt — de feiten over de wijze van bekendmaken horen bij de
    /// bekendmaking zelf, en de wet zegt welke dat zijn.
    ///
    /// In de versie die op `moment` gold: een bekendmaking rekent onder het
    /// recht van het besluit, dus ook haar formulier komt uit dat recht.
    pub(crate) fn bekendmaking_procedure(
        &self,
        besluit: &str,
        moment: NaiveDate,
    ) -> Result<BekendmakingProcedure> {
        let definition = self.definition(besluit)?;
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
        let service = service.borrow();
        let resolver = service.resolver();
        let produces = resolver
            .get_article_by_output(&definition.regulation, &definition.output, Some(moment))
            .and_then(regelrecht_engine::Article::get_execution_spec)
            .and_then(|execution| execution.produces.as_ref());
        let geen_stage = |reason: String| SimulatorError::GeenBekendmakingStage {
            cell: self.id.clone(),
            besluit: definition.name.clone(),
            legal_character: BESCHIKKING.to_string(),
            stage: besluit::STAGE_BEKENDMAKING.to_string(),
            reason,
        };
        let procedure = resolver
            .find_procedure(
                BESCHIKKING,
                produces.and_then(|p| p.procedure_id.as_deref()),
            )
            .ok_or_else(|| geen_stage("geen procedure gevonden".to_string()))?;
        let Some(stage) = procedure
            .stages
            .iter()
            .find(|stage| stage.name == besluit::STAGE_BEKENDMAKING)
        else {
            return Err(geen_stage(format!(
                "procedure '{}' kent de stages {}",
                procedure.id,
                procedure
                    .stages
                    .iter()
                    .map(|stage| stage.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };

        let mut form = Vec::new();
        for requirement in stage.requires.iter().flatten() {
            use regelrecht_engine::ParameterType as Type;
            let value_type = match requirement.req_type {
                Type::String => ParameterType::String,
                Type::Number => ParameterType::Number,
                Type::Amount => ParameterType::Amount,
                Type::Boolean => ParameterType::Boolean,
                Type::Date => ParameterType::Date,
                other @ (Type::Array | Type::Object) => {
                    return Err(SimulatorError::BekendmakingVeldType {
                        cell: self.id.clone(),
                        besluit: definition.name.clone(),
                        veld: requirement.name.clone(),
                        type_name: output_type_name(other).to_string(),
                    })
                }
            };
            form.push(DocumentedParameter {
                name: requirement.name.clone(),
                value_type,
                // Geen opgave: een datumveld krijgt dan de klok (zie
                // `snapshot::prefilled`), en de rest weet de wereld niet.
                prefill: None,
            });
        }
        // De dag van de bekendmaking gaat de wet in onder de naam die de
        // procedure ervoor kiest: het eerste datumveld van `requires`.
        let datumveld = form
            .iter()
            .find(|field| field.value_type == ParameterType::Date)
            .map(|field| field.name.clone())
            .ok_or_else(|| SimulatorError::BekendmakingZonderDatumveld {
                cell: self.id.clone(),
                besluit: definition.name.clone(),
                procedure: procedure.id.clone(),
            })?;
        Ok(BekendmakingProcedure {
            procedure_id: procedure.id.clone(),
            form,
            datumveld,
        })
    }

    /// De hooks die bij dit besluit op deze stage vuren en een verplichte
    /// parameter vragen die daar niet bestaat.
    ///
    /// De **optuigtoets** bij [`HookNietUitgevoerd`]: zo'n hook laat het besluit
    /// niet omvallen, en juist daarom hoort het gat bij het optuigen al te zien
    /// te zijn — anders is een corpusfout pas te vinden in een gram. Een droge
    /// toets op wat vooruit vaststaat, over elke geladen versie: wat de engine
    /// een hook-artikel kan geven (de inputs van het besluit, de parameters,
    /// inputs en — na de acties — uitkomsten van het uitvoerende artikel, en bij
    /// de bekendmaking ook de uitkomsten van het besluit, het bevoegd gezag en
    /// de `requires` van de stage), tegen de verplichte parameters van het
    /// hook-artikel zelf. Wat een hook dieper in de wet leest, blijkt pas bij de
    /// uitvoering; dan staat het in het gram en in het journaal.
    pub(crate) fn hooks_zonder_input(&self, besluit: &str, stage: &str) -> Vec<HookZonderInput> {
        let (Some(definition), Some(service)) =
            (self.besluiten.get(besluit), self.besluit_service.as_ref())
        else {
            return Vec::new();
        };
        let service = service.borrow();
        let resolver = service.resolver();
        let mut gaps = BTreeSet::new();
        for law in resolver
            .all_law_versions()
            .filter(|law| law.id == definition.regulation)
        {
            let Some(execution) = law
                .articles
                .iter()
                .filter_map(Article::get_execution_spec)
                .find(|execution| {
                    execution
                        .output
                        .iter()
                        .flatten()
                        .any(|output| output.name == definition.output)
                })
            else {
                continue;
            };
            let Some(produces) = execution.produces.as_ref() else {
                continue;
            };
            let Some(legal_character) = produces.legal_character.as_deref() else {
                continue;
            };
            let mut before: BTreeSet<String> = definition.inputs.keys().cloned().collect();
            before.extend(
                execution
                    .parameters
                    .iter()
                    .flatten()
                    .map(|p| p.name.clone()),
            );
            before.extend(execution.input.iter().flatten().map(|i| i.name.clone()));
            if stage == besluit::STAGE_BEKENDMAKING {
                before.extend(
                    definition
                        .recorded_outputs()
                        .into_iter()
                        .map(str::to_string),
                );
                before.insert(besluit::COMPETENT_AUTHORITY.to_string());
                if let Some(procedure) =
                    resolver.find_procedure(legal_character, produces.procedure_id.as_deref())
                {
                    before.extend(
                        procedure
                            .stages
                            .iter()
                            .filter(|candidate| candidate.name == stage)
                            .flat_map(|candidate| candidate.requires.iter().flatten())
                            .map(|requirement| requirement.name.clone()),
                    );
                }
            }
            let mut after = before.clone();
            after.extend(execution.output.iter().flatten().map(|o| o.name.clone()));

            for hook_law in resolver.all_law_versions() {
                for hook_article in &hook_law.articles {
                    for hook in hook_article.get_hooks().into_iter().flatten() {
                        let filter = &hook.applies_to;
                        let fires = filter.legal_character.as_deref() == Some(legal_character)
                            && filter.stage.as_deref().unwrap_or(besluit::STAGE_BESLUIT) == stage
                            && filter.decision_type.as_ref().is_none_or(|wanted| {
                                produces.decision_type.as_ref() == Some(wanted)
                            });
                        if !fires {
                            continue;
                        }
                        let available = match hook.hook_point {
                            regelrecht_engine::article::HookPoint::PreActions => &before,
                            regelrecht_engine::article::HookPoint::PostActions => &after,
                        };
                        let Some(hook_execution) = hook_article.get_execution_spec() else {
                            continue;
                        };
                        for parameter in hook_execution.parameters.iter().flatten() {
                            if parameter.required == Some(true)
                                && !available.contains(&parameter.name)
                            {
                                gaps.insert(HookZonderInput {
                                    regulation: hook_law.id.clone(),
                                    article: hook_article.number.clone(),
                                    input: parameter.name.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
        gaps.into_iter().collect()
    }

    /// Maak het laatste besluit van deze cel bekend, en leg dat vast.
    ///
    /// Dit is de tweede stage van de procedure die de algemene wet voor een
    /// beschikking declareert (RFC-008): hetzelfde zaakkenmerk, hetzelfde
    /// besluit, een eigen elementair gram (RFC-022 §1.2). Er wordt **niets
    /// opnieuw vastgesteld** — het besluit is genomen en een kroniek wijzigt
    /// niet — maar er komt wel iets bij: wat de wet aan de bekendmaking hangt.
    /// Dat komt uit de **hooks** die op die stage vuren (de algemene wet), en
    /// uit de uitkomsten die de eigen regeling voor deze stage declareert
    /// (`stage_uitkomsten`).
    ///
    /// Het formulier is wat de procedure in `requires` van de stage vraagt
    /// ([`Self::bekendmaking_procedure`]). Die waarden gaan als parameters de
    /// stage in en komen als inputs met herkomst `parameter` in het gram; het
    /// eerste datumveld is de dag van de bekendmaking, en dat is de klok.
    ///
    /// De stage draait op de inputs die in het gram staan en op het moment van
    /// het **besluit**: onder het recht van toen, op de feiten van toen. Dat is
    /// geen detail maar de reden dat dit geen tweede besluit is — zou de stage
    /// op vandaag rekenen, dan kon een bekendmaking een ander bedrag opleveren
    /// dan het besluit dat ze bekendmaakt. Wat er wél van vandaag is, staat in
    /// het formulier.
    ///
    /// Geen enkele vraag gaat hier over een celgrens: de stage krijgt geen
    /// cel-resolver, want wat er van een ander geaccepteerd is, staat al in het
    /// gram.
    pub(crate) fn bekendmaken(
        &mut self,
        besluit: &str,
        identity: &str,
        op_moment: NaiveDate,
        form_values: &BTreeMap<String, Value>,
    ) -> Result<Bekendmaking> {
        let definition = self.definition(besluit)?;
        let stand = self.bekendmaking_stand(besluit);
        let BekendmakingStand::TeDoen {
            zaakkenmerk,
            plek,
            besluit_op_moment,
        } = stand
        else {
            return Err(SimulatorError::BekendmakingNietMogelijk {
                cell: self.id.clone(),
                besluit: besluit.to_string(),
                reason: stand.describe(),
            });
        };

        // Het formulier, getoetst zoals elk formulier: een veld dat ontbreekt of
        // van het verkeerde type is, weigert de actie met het veld erbij.
        let procedure = self.bekendmaking_procedure(besluit, besluit_op_moment)?;
        check_documented_params(
            &self.id,
            Subject::Besluit,
            &definition.name,
            &procedure.form,
            form_values,
        )?;
        let dag = form_values
            .get(&procedure.datumveld)
            .and_then(Value::as_str)
            .unwrap_or_default();
        if dag != op_moment.to_string() {
            return Err(SimulatorError::BekendmakingDatumNietDeKlok {
                cell: self.id.clone(),
                besluit: besluit.to_string(),
                veld: procedure.datumveld.clone(),
                value: dag.to_string(),
                clock: op_moment.to_string(),
            });
        }

        // Wat het gram draagt, eerst eruit: de uitvoering hieronder leunt erop,
        // en het gram zelf blijft waar het is.
        let Some(gram) = self.gram(BESCHIKKINGEN, plek) else {
            return Err(SimulatorError::BekendmakingNietMogelijk {
                cell: self.id.clone(),
                besluit: besluit.to_string(),
                reason: format!("het gram op plek {plek} is er niet meer"),
            });
        };
        let outputs_van_besluit: BTreeMap<String, Value> = definition
            .recorded_outputs()
            .into_iter()
            .filter_map(|name| {
                gram.fields
                    .get(name)
                    .map(|value| (name.to_string(), value.clone()))
            })
            .collect();
        let mut stage_params: BTreeMap<String, Value> = besluit::recorded_inputs(&gram.fields);
        let competent_authority = gram
            .fields
            .get(besluit::COMPETENT_AUTHORITY)
            .and_then(Value::as_str)
            .map(str::to_string);
        let decision_type = gram
            .fields
            .get(besluit::DECISION_TYPE)
            .and_then(Value::as_str)
            .map(str::to_string);
        let wachtend = besluit::wachtende_verplichtingen(&gram.fields).ok_or_else(|| {
            SimulatorError::OnleesbareWachtendeVerplichting {
                cell: self.id.clone(),
                besluit: besluit.to_string(),
            }
        })?;

        // Wat er bij deze stage bij komt, naast de inputs van het besluit: het
        // formulier van de procedure, en het bevoegd gezag, dat een
        // rechtsmiddelenclausule nodig heeft om te kunnen zeggen bij wie bezwaar
        // gemaakt moet worden. Een hook die ze niet declareert, krijgt ze ook
        // niet: de engine geeft elk hook-artikel alleen wat het zelf noemt.
        let inputs: BTreeMap<String, DecretogramInput> = form_values
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    DecretogramInput {
                        value: value.clone(),
                        origin: InputOrigin::Parameter {
                            parameter: name.clone(),
                        },
                    },
                )
            })
            .collect();
        stage_params.extend(
            form_values
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );
        if let Some(authority) = &competent_authority {
            stage_params.insert(
                besluit::COMPETENT_AUTHORITY.to_string(),
                Value::String(authority.clone()),
            );
        }

        let uitkomst = self.run_stage(
            &definition,
            &outputs_van_besluit,
            stage_params,
            besluit_op_moment,
            procedure.procedure_id,
        )?;

        // Valt er nog iets te beloven? Een verplichting met `vanaf: bekendmaking`
        // gaat pas op dit moment werken (Awb 3:40), dus dit is de plek waar
        // blijkt dat ze dat niet meer doet: ligt er over dezelfde zaak inmiddels
        // een beschikking die in de plaats van deze kwam, dan wordt er niets
        // ingeroosterd. Bij het besluit kon dat niet blijken — daar staat de
        // belofte in een gram en niet in een wachtrij, en een gram verandert
        // niet.
        let termijnen_vervallen_door = if wachtend.is_empty() {
            None
        } else {
            self.vervangen_na(plek, &zaakkenmerk)?
        };

        // De termijnen die op deze bekendmaking wachtten. De eerste vervaldag is
        // de uitkomst die de verplichting zelf als `vervaldatum` noemt, en die
        // de wet zojuist uitrekende — niet een dag die deze opstelling bedacht.
        // Wat vóór vandaag zou vervallen, wordt vandaag ingehaald: vóór de
        // bekendmaking werkt het besluit niet.
        let mut obligations = Vec::new();
        if !wachtend.is_empty() && termijnen_vervallen_door.is_none() {
            let zaak = BesluitGram {
                cell: &self.id,
                besluit: &definition.name,
                zaakkenmerk: &zaakkenmerk,
                op_moment: besluit_op_moment,
                plek,
            };
            for verplichting in &wachtend {
                let veld = &verplichting.vervaldatum_uit;
                let start = uitkomst
                    .outputs
                    .get(veld)
                    .and_then(Value::as_str)
                    .and_then(|text| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok())
                    .ok_or_else(|| SimulatorError::BekendmakingZonderBetaaldatum {
                        cell: self.id.clone(),
                        besluit: besluit.to_string(),
                        veld: veld.clone(),
                        found: uitkomst.listing(),
                    })?;
                obligations.extend(verplichting.termijnen_vanaf(start, op_moment, &zaak)?);
            }
        }

        let bekendmaking = Bekendmaking {
            cell: self.id.clone(),
            inputs,
            stage_uitkomsten: uitkomst.stage_uitkomsten,
            hooks_niet_uitgevoerd: uitkomst.hooks_niet_uitgevoerd,
            besluit: definition.name.clone(),
            zaakkenmerk,
            op_moment,
            besluit_op_moment,
            besluit_gram: plek,
            regulation: definition.regulation.clone(),
            regulation_valid_from: uitkomst.regulation_valid_from,
            competent_authority,
            bekendgemaakt_door: identity.to_string(),
            legal_character: BESCHIKKING.to_string(),
            decision_type,
            outputs: uitkomst.outputs,
            hooks: uitkomst.hooks,
            obligations,
            termijnen_vervallen_door,
            receipt: uitkomst.receipt,
        };

        let event = bekendmaking.event()?;
        self.record_own(BESCHIKKINGEN, event)?;
        Ok(bekendmaking)
    }

    /// Voer één stage van de procedure uit die bij dit besluit hoort.
    ///
    /// De engine doet het werk: zij indexeert de hooks per stage en weet welke
    /// artikelen er vuren (RFC-008). Wat deze cel aanlevert is de stand waarin
    /// het besluit verkeert — de uitkomsten die het opleverde en de parameters
    /// waarop het rekende — als [`StageState`], plus wat er bij deze stage bij
    /// komt.
    ///
    /// Daarna de **eigen regeling**: de uitkomsten die het uitvoerende artikel
    /// onder `stage_uitkomsten` voor deze stage declareert, uitgerekend op
    /// dezelfde parameters en hetzelfde moment. Een hook kan dat niet — hij
    /// vuurt op elke beschikking van een soort — en een uitkomst van het
    /// besluit-gram komt hier nooit opnieuw uit: het optuigen weigert een
    /// stage-uitkomst die het besluit al vastlegt.
    ///
    /// Mét trace, om dezelfde reden als bij een besluit: zonder haar staat er wel
    /// wát er uitkwam, maar niet langs welke artikelen (RFC-013). `new_untimed`,
    /// zodat twee bekendmakingen met dezelfde invoer hetzelfde gram opleveren.
    fn run_stage(
        &self,
        definition: &BesluitDefinition,
        outputs_van_besluit: &BTreeMap<String, Value>,
        parameters: BTreeMap<String, Value>,
        besluit_op_moment: NaiveDate,
        procedure_id: String,
    ) -> Result<StageUitkomst> {
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
        // De eigen kronieken zoals ze op het moment van het besluit waren: wat de
        // regeling zelf uit een databron haalt, hoort bij de bekendmaking niet
        // opeens nieuwer te zijn dan bij het besluit.
        self.register_own_facts(&mut service, besluit_op_moment)?;

        let state = StageState {
            procedure_id,
            contextual_law: definition.regulation.clone(),
            current_stage: besluit::STAGE_BEKENDMAKING.to_string(),
            accumulated_outputs: outputs_van_besluit.clone(),
            parameters: parameters.clone(),
            accepted_values: Vec::new(),
        };
        let calculation_date = besluit_op_moment.format("%Y-%m-%d").to_string();
        let outcome = service
            .execute_stage_with_trace_builder(
                &definition.regulation,
                &definition.output,
                Some(state),
                parameters.clone(),
                &calculation_date,
                TraceBuilder::new_untimed(),
            )
            .map_err(|error| self.explain_undeclared_source(definition, untraced(error)))?;

        let mut result = match outcome {
            ExecutionOutcome::Complete(result) => *result,
            ExecutionOutcome::Yielded { pending_inputs, .. } => {
                return Err(SimulatorError::BekendmakingWachtOpInvoer {
                    cell: self.id.clone(),
                    besluit: definition.name.clone(),
                    missing: pending_inputs.join(", "),
                })
            }
        };

        // Alleen wat de **hooks** op deze stage opleverden. De uitkomsten van het
        // besluit staan al in zijn eigen gram, en een stage-gram dat ze nog eens
        // draagt zou twee keer hetzelfde zeggen — en bij de eerste afwijking niet
        // meer te lezen zijn als wat het is.
        let mut outputs = BTreeMap::new();
        let mut hooks = Vec::new();
        let resolver = service.resolver();
        for (name, provenance) in &result.output_provenance {
            let OutputProvenance::Reactive {
                law_id,
                article,
                hook_point,
            } = provenance
            else {
                continue;
            };
            let Some(value) = result.outputs.get(name) else {
                continue;
            };
            outputs.insert(name.clone(), value.clone());
            hooks.push(HookHerkomst {
                veld: name.clone(),
                lexogram: ObligationOrigin {
                    regulation: law_id.clone(),
                    valid_from: resolver
                        .get_law_for_date(law_id, Some(besluit_op_moment))
                        .and_then(|law| law.valid_from.clone()),
                    article: article.clone(),
                },
                hook_point: hook_point.clone(),
            });
        }
        let hooks_niet_uitgevoerd =
            hooks_niet_uitgevoerd(resolver, &result.skipped_hooks, besluit_op_moment);

        // En wat de eigen regeling voor deze stage declareert. Uit de versie die
        // op het moment van het besluit gold, met dezelfde lezer als het
        // optuigen.
        let (_, origin, produces) = executing_article(
            resolver,
            &definition.regulation,
            &definition.output,
            besluit_op_moment,
        );
        let eigen: Vec<String> = ChronolexBlock::read(produces, &origin)?
            .stage_uitkomsten_voor(besluit::STAGE_BEKENDMAKING)
            .to_vec();
        let mut stage_uitkomsten = Vec::new();
        if !eigen.is_empty() {
            let requested: Vec<&str> = eigen.iter().map(String::as_str).collect();
            let own = service
                .evaluate_law_with_trace_builder(
                    &definition.regulation,
                    &requested,
                    parameters.clone(),
                    &calculation_date,
                    TraceBuilder::new_untimed(),
                )
                .map_err(|error| self.explain_undeclared_source(definition, untraced(error)))?;
            let resolver = service.resolver();
            for name in &eigen {
                let Some(value) = own.outputs.get(name) else {
                    continue;
                };
                outputs.insert(name.clone(), value.clone());
                let article = match own.output_provenance.get(name) {
                    Some(
                        OutputProvenance::Direct { article, .. }
                        | OutputProvenance::Override { article, .. }
                        | OutputProvenance::Reactive { article, .. },
                    ) => article.clone(),
                    None => resolver
                        .get_article_by_output(
                            &definition.regulation,
                            name,
                            Some(besluit_op_moment),
                        )
                        .map(|article| article.number.clone())
                        .unwrap_or_default(),
                };
                stage_uitkomsten.push(StageUitkomstHerkomst {
                    veld: name.clone(),
                    lexogram: ObligationOrigin {
                        regulation: definition.regulation.clone(),
                        valid_from: own.regulation_valid_from.clone(),
                        article,
                    },
                });
                if let Some(provenance) = own.output_provenance.get(name) {
                    result
                        .output_provenance
                        .insert(name.clone(), provenance.clone());
                }
                result.outputs.insert(name.clone(), value.clone());
            }
            // Eén receipt voor het hele gram: de uitvoering van de eigen
            // regeling hangt als tak onder de stage, zodat de trace beide laat
            // zien en geen van de twee het gram zonder herkomst verlaat.
            if let Some(own_trace) = own.trace {
                match result.trace.as_mut() {
                    Some(trace) => trace.children.push(own_trace),
                    None => result.trace = Some(own_trace),
                }
            }
        }

        let requested: Vec<String> = outputs.keys().cloned().collect();
        let receipt =
            service.build_receipt_with_outputs(&result, &parameters, &calculation_date, &requested);
        Ok(StageUitkomst {
            outputs,
            hooks,
            stage_uitkomsten,
            hooks_niet_uitgevoerd,
            regulation_valid_from: result.regulation_valid_from.clone(),
            receipt,
        })
    }

    /// Wat dit besluit bij een andere cel moet ophalen voordat het kan rekenen.
    ///
    /// De cel stelt de vragen samen en stelt ze niet: ze kan geen andere cel
    /// bereiken. Wie dat wel kan — in deze opstelling [`crate::World::decide`] —
    /// haalt de antwoorden op en geeft ze aan [`Self::decide`] terug. Dat die
    /// twee stappen buiten de cel bij elkaar komen, is geen omweg maar de vorm
    /// van RFC-022 §2.
    ///
    /// Dat die splitsing de weigeringen van [`Self::decide`] niet mag verschuiven,
    /// is de reden dat het zaakkenmerk hier al langskomt. Een vraag over een
    /// celgrens is bij de bevraagde organisatie een gebeurtenis — zij ziet wie er
    /// iets over wie kwam opvragen — en die hoort niet te vallen voor een besluit
    /// dat om zijn eigen kenmerk toch al niet genomen kan worden. Zonder deze
    /// regel zou de orde van `decide` ("eerst het kenmerk, dan ophalen en
    /// rekenen") wél in de cel staan en niet meer gelden.
    ///
    /// Om diezelfde reden komt ook de toets op het bevoegd gezag hier al langs.
    /// Een cel die de regeling niet mag uitvoeren, hoort een andere organisatie
    /// niet eerst te laten zien dat er iets over iemand werd opgevraagd.
    pub(crate) fn acceptance_requests(
        &self,
        besluit: &str,
        identity: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<Vec<AcceptanceRequest>> {
        let definition = self.definition(besluit)?;
        definition.check_params(&self.id, params)?;
        let zaakkenmerk = definition.zaakkenmerk(&self.id, params)?;
        let authority = self.competent_authority(&definition, op_moment)?;
        self.check_competent_authority(&definition, identity, authority.as_deref())?;
        Ok(definition.acceptance_requests(params, &zaakkenmerk))
    }

    /// Het bevoegd gezag dat de regeling van dit besluit noemt, op dit moment.
    ///
    /// Uit het **law-model** en niet uit de celconfiguratie: de wet bepaalt wie
    /// het bevoegd gezag is. `None` betekent dat de regeling er niets over zegt
    /// — een gat in die regeling, niet iets dat het platform invult.
    ///
    /// RFC-002 legt het gezag op het **artikel**: één wet kan nul tot veel
    /// bevoegde gezagen kennen, en het artikel dat de aansturende uitkomst
    /// voortbrengt zegt wie dít besluit mag nemen. Het documentniveau is de
    /// terugvaloptie — het schema laat het toe en het corpus gebruikt het — maar
    /// zodra het artikel zelf iets zegt, gaat dat voor. Een wet waarin de ene
    /// dienst indiceert en de andere betaalt, zou anders voor élk besluit één
    /// gezag noemen, en dat is niet wat de wet zegt.
    ///
    /// Op `op_moment` en niet op vandaag, want het is een eigenschap van de
    /// versie die toen gold: verandert de wet van gezag, dan blijft een besluit
    /// van toen door het gezag van toen genomen.
    ///
    /// `Ok(None)` is een regeling die zwijgt. Een regeling die wél iets
    /// declareert maar met een `#`-verwijzing die nergens op uitkomt, is een
    /// fout: doorgaan alsof ze zwijgt zou de toets stil uitzetten, en dan
    /// besluit iedereen.
    fn competent_authority(
        &self,
        definition: &BesluitDefinition,
        op_moment: NaiveDate,
    ) -> Result<Option<String>> {
        let Some(service) = self.besluit_service.as_ref() else {
            return Ok(None);
        };
        let service = service.borrow();
        let resolver = service.resolver();
        let Some(law) = resolver.get_law_for_date(&definition.regulation, Some(op_moment)) else {
            return Ok(None);
        };
        let article = resolver.get_article_by_output(
            &definition.regulation,
            &definition.output,
            Some(op_moment),
        );
        let Some(declared) = declared_authority(law, article) else {
            return Ok(None);
        };
        match competent_authority(law, declared) {
            Some(name) => Ok(Some(name)),
            None => Err(SimulatorError::CompetentAuthorityUnresolvable {
                cell: self.id.clone(),
                besluit: definition.name.clone(),
                regulation: definition.regulation.clone(),
                reference: match declared {
                    CompetentAuthority::String(text) => {
                        text.strip_prefix('#').unwrap_or(text).to_string()
                    }
                    CompetentAuthority::Structured { name } => name.clone(),
                },
            }),
        }
    }

    /// Wat het **lexogram** dit besluit oplegt, in de versie die op dit moment
    /// gold.
    ///
    /// Het artikel dat de sturende uitkomst voortbrengt is de plek: díe uitkomst
    /// *is* het besluit, en hetzelfde artikel zegt ook wie het mag nemen
    /// (RFC-002). Dezelfde opzoeking als bij [`Self::competent_authority`], op
    /// dezelfde versie — anders kon een gram een verplichting dragen uit een
    /// andere versie dan die waaronder besloten is.
    ///
    /// Een artikel zonder blok legt niets op, en dat is geen fout: een
    /// beschikking zonder bedrag is een gewone beschikking.
    fn declared_obligations(
        &self,
        definition: &BesluitDefinition,
        op_moment: NaiveDate,
    ) -> Result<DeclaredObligations> {
        let none = || DeclaredObligations::none(&definition.regulation);
        let Some(service) = self.besluit_service.as_ref() else {
            return Ok(none());
        };
        let service = service.borrow();
        let resolver = service.resolver();
        let Some(law) = resolver.get_law_for_date(&definition.regulation, Some(op_moment)) else {
            return Ok(none());
        };
        let Some(article) = resolver.get_article_by_output(
            &definition.regulation,
            &definition.output,
            Some(op_moment),
        ) else {
            return Ok(none());
        };
        DeclaredObligations::from_article(
            ObligationOrigin {
                regulation: law.id.clone(),
                valid_from: law.valid_from.clone(),
                article: article.number.clone(),
            },
            declared_authority(law, Some(article))
                .and_then(|declared| competent_authority(law, declared)),
            article,
        )
    }

    /// Per uitkomst het artikel dat haar voortbrengt, in de versie die op
    /// `op_moment` gold.
    ///
    /// Voor de herkomst van een ritme dat uit een uitkomst komt: die uitkomst kan
    /// uit een ander artikel komen dan het artikel dat de verplichting declareert,
    /// en het gram hoort het artikel te noemen dat het ritme uitrekende. Langs de
    /// resolver van de engine, dezelfde weg als bij een afwijzingsgrond. Een
    /// uitkomst zonder artikel staat er niet in.
    fn output_articles<'a>(
        &self,
        regulation: &str,
        outputs: impl Iterator<Item = &'a str>,
        op_moment: NaiveDate,
    ) -> BTreeMap<String, String> {
        let Some(service) = self.besluit_service.as_ref() else {
            return BTreeMap::new();
        };
        let service = service.borrow();
        let resolver = service.resolver();
        outputs
            .filter_map(|output| {
                resolver
                    .get_article_by_output(regulation, output, Some(op_moment))
                    .map(|article| (output.to_string(), article.number.clone()))
            })
            .collect()
    }

    /// Wat het artikel van dit besluit zegt over de termijnen die over dezelfde
    /// zaak nog openstaan.
    ///
    /// `None` is: het zegt er niets over, en dan blijft staan wat er staat — een
    /// verplichting is niet in te trekken. Staat er wél iets, dan komt deze
    /// beschikking in de plaats van de vorige en vervallen de termijnen die nog
    /// niet verstreken waren (zie [`Vervanging`]).
    ///
    /// De cel leest het en handelt er niet naar: zij kent geen klok en geen
    /// wachtrij. Wat er met een ingeroosterde termijn gebeurt, is aan de wereld
    /// waarin die termijn vervalt.
    ///
    /// Op `op_moment` en langs dezelfde weg als [`Self::declared_obligations`]:
    /// het staat in hetzelfde blok van hetzelfde artikel, dus het hoort uit
    /// dezelfde versie te komen als de verplichtingen waarover het gaat.
    pub(crate) fn vervanging(
        &self,
        besluit: &str,
        op_moment: NaiveDate,
    ) -> Result<Option<Vervanging>> {
        let definition = self.definition(besluit)?;
        Ok(self
            .declared_obligations(&definition, op_moment)?
            .vervanging)
    }

    /// Wat de geladen versies van haar wetten aan dit besluit opleggen.
    ///
    /// Voor het **optuigen** van de wereld: die toetst of er een cel aan het
    /// gezag gebonden is en of beide kanten een betalingsstroom houden. Alle
    /// versies, want een besluit over een ouder moment landt op een oudere
    /// versie.
    pub(crate) fn obligation_declarations(
        &self,
        definition: &BesluitDefinition,
    ) -> &[DeclaredObligations] {
        self.obligations
            .get(&definition.regulation)
            .and_then(|outputs| outputs.get(&definition.output))
            .map_or(&[], Vec::as_slice)
    }

    /// Is deze cel het bevoegd gezag van de regeling die ze wil uitvoeren?
    ///
    /// De vergelijking gaat over **genormaliseerde tekst** (zie [`normalised`]):
    /// een spatie vooraan of een hoofdletter is een schrijfwijze en geen andere
    /// organisatie. Verder wordt er niets geïnterpreteerd — een afkorting is een
    /// andere naam, en die hoort in het wereldbestand of in de wet rechtgezet te
    /// worden en niet hier geraden.
    ///
    /// Noemt de regeling geen bevoegd gezag, dan is er niets te toetsen en gaat
    /// het besluit door. Dat is een keuze: de opstelling blokkeren op een gat in
    /// een regeling zou de speeltuin dichtzetten voor iets waar de cel niets aan
    /// kan doen. Het gram draagt dan `competent_authority: null` en de wereld
    /// waarschuwt erover, zodat het gat zichtbaar is in plaats van stil.
    fn check_competent_authority(
        &self,
        definition: &BesluitDefinition,
        identity: &str,
        authority: Option<&str>,
    ) -> Result<()> {
        let Some(authority) = authority else {
            return Ok(());
        };
        if normalised(authority) == normalised(identity) {
            return Ok(());
        }
        Err(SimulatorError::NotCompetentAuthority {
            cell: self.id.clone(),
            besluit: definition.name.clone(),
            identity: identity.to_string(),
            regulation: definition.regulation.clone(),
            authority: authority.to_string(),
        })
    }

    /// Eén besluit-definitie van deze cel, of een nette fout die opsomt wat de
    /// cel wél kan besluiten.
    ///
    /// `pub(crate)`: een actie die een besluit-pad start, moet bij het optuigen
    /// kunnen vaststellen dat dat besluit bestaat, en moet het formulier ervan
    /// kunnen tonen — de gedocumenteerde parameters van het besluit. Een tweede
    /// lijst parameters in de actie zou uiteen gaan lopen met de eerste.
    pub(crate) fn besluit_definition(&self, besluit: &str) -> Result<BesluitDefinition> {
        self.definition(besluit)
    }

    /// Eén besluit-definitie van deze cel, of een nette fout die opsomt wat de
    /// cel wél kan besluiten.
    fn definition(&self, besluit: &str) -> Result<BesluitDefinition> {
        Ok(self
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
            .clone())
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
        accepted: &BTreeMap<String, DecretogramInput>,
        zaakkenmerk: &str,
        op_moment: NaiveDate,
    ) -> Result<BTreeMap<String, DecretogramInput>> {
        let mut collected: BTreeMap<String, DecretogramInput> = BTreeMap::new();
        for (input, origin) in &definition.inputs {
            let gathered = match origin {
                BesluitInput::AcceptFrom {
                    cell, lexostatus, ..
                } => {
                    // Onbereikbaar langs `World::decide`, dat elk verzoek uit
                    // `acceptance_requests` inwilligt voordat het hier komt. Een
                    // ontbrekend antwoord mag hier nooit stil een gat worden: dan
                    // zou de engine op `null` rekenen en zou er een besluit
                    // liggen dat een feit mist.
                    accepted.get(input).cloned().ok_or_else(|| {
                        SimulatorError::BesluitInputMissing {
                            cell: self.id.clone(),
                            besluit: definition.name.clone(),
                            input: input.clone(),
                            reason: format!(
                                "lexostatus '{lexostatus}' van cel '{cell}' is voor dit \
                                 besluit niet opgehaald"
                            ),
                        }
                    })?
                }
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
                BesluitInput::FromDecretogram {
                    besluit: earlier,
                    field,
                } => self.read_earlier_decretogram(
                    definition,
                    input,
                    earlier,
                    field,
                    zaakkenmerk,
                    op_moment,
                )?,
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
        // Onbereikbaar leeg: `validate` eist dat de stroom bestaat en dat haar
        // sleutelveld een gedocumenteerde parameter is.
        let key = self.chronicles.key_of(chronicle).unwrap_or_default();
        let missing = |reason: String| SimulatorError::BesluitInputMissing {
            cell: self.id.clone(),
            besluit: definition.name.clone(),
            input: input.to_string(),
            reason,
        };
        // Langs [`Self::decide`] ligt die parameter er altijd — `check_params`
        // eist hem — maar langs de droogloop van [`Self::missing_own_fact`] niet:
        // daar staat het formulier met zijn voorinvulling, en een voorinvulling
        // die nergens op uitkomt levert geen waarde. Dat is geen defect maar het
        // antwoord zelf, dus de reden noemt de stroom waar het onderwerp uit zou
        // moeten komen en niet de parameter als administratief gat.
        let key_value = params.get(key).ok_or_else(|| {
            missing(format!(
                "de vraag noemt nog geen {key}, dus er is in kroniekstroom \
                 '{chronicle}' geen vastlegging aan te wijzen"
            ))
        })?;

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

    /// Eén input uit een **eerder besluit** van deze cel over dezelfde zaak.
    ///
    /// De sleutel is het zaakkenmerk van het lopende besluit en komt dus uit het
    /// eigen sjabloon: een vaststelling leest de verlening over déze zaak terug.
    /// Van de grammen met die naam en dat kenmerk wint het laatste op of vóór het
    /// moment van dit besluit — dezelfde regel als bij een eigen kroniek, want
    /// het is dezelfde tijdas.
    ///
    /// Ligt er geen zo'n gram, dan valt het besluit om en wordt er niets
    /// vastgelegd. Dat is geen "niets vastgesteld": een vaststelling zonder de
    /// verlening waarop ze terugslaat, hoort niet met een gat verder te rekenen.
    fn read_earlier_decretogram(
        &self,
        definition: &BesluitDefinition,
        input: &str,
        earlier: &str,
        field: &str,
        zaakkenmerk: &str,
        op_moment: NaiveDate,
    ) -> Result<DecretogramInput> {
        let missing = |reason: String| SimulatorError::BesluitInputMissing {
            cell: self.id.clone(),
            besluit: definition.name.clone(),
            input: input.to_string(),
            reason,
        };
        let key_value = Value::String(zaakkenmerk.to_string());
        // De naam van het gram is de naam van de besluit-definitie (zie
        // [`Decretogram::event`]), en die staat óók als veld in het gram. Op het
        // veld filteren en niet op de naam: dat is de weg die elke andere
        // reductie over deze stroom ook gaat.
        let conditions = BTreeMap::from([
            (
                besluit::BESLUIT.to_string(),
                Value::String(earlier.to_string()),
            ),
            // En van dát besluit de **stage BESLUIT**: over dezelfde zaak liggen
            // meer grammen van hetzelfde besluit — de bekendmaking is er ook een
            // (RFC-008) — en wie terugleest, leest het besluit terug en niet de
            // stage die erna kwam. Zonder dit filter zou een vaststelling na een
            // bekendmaking stil op het verkeerde gram landen.
            (
                besluit::STAGE.to_string(),
                Value::String(besluit::STAGE_BESLUIT.to_string()),
            ),
        ]);
        let event = self
            .chronicles
            .latest_recording(
                BESCHIKKINGEN,
                besluit::ZAAKKENMERK,
                &key_value,
                &conditions,
                op_moment,
            )
            .ok_or_else(|| {
                missing(format!(
                    "geen eerder besluit '{earlier}' voor zaak '{zaakkenmerk}' \
                     op of vóór {op_moment}"
                ))
            })?;

        // Twee lagen, en de volgorde doet er niet toe: de velden van het gram
        // zelf — de uitkomsten en de vaste velden — en de inputs waarop dat
        // besluit rekende, die een laag dieper liggen met elk hun eigen herkomst.
        // Een naam die in beide lagen zit is bij het optuigen geweigerd
        // ([`SimulatorError::AmbiguousEarlierBesluitField`]), dus hier kan er
        // hoogstens één van de twee iets opleveren. Wat meekomt is de waarde; dat
        // er teruggelezen is, staat in de herkomst hieronder.
        let value = chronicle::field(&event.fields, field)
            .or_else(|| recorded_input(&event.fields, field))
            .cloned()
            .ok_or_else(|| {
                missing(format!(
                    "het besluit '{earlier}' van {} over zaak '{zaakkenmerk}' draagt \
                     veld '{field}' niet",
                    event.op_moment
                ))
            })?;

        Ok(DecretogramInput {
            value,
            origin: InputOrigin::EarlierDecretogram {
                besluit: earlier.to_string(),
                zaakkenmerk: zaakkenmerk.to_string(),
                moment: event.op_moment,
            },
        })
    }

    /// Welk **eigen** feit dit besluit op dit moment mist; `None` als ze er
    /// allemaal liggen.
    ///
    /// Een droogloop van de inputresolutie hierboven, en daarmee het antwoord op
    /// "kan dit besluit nu genomen worden?". Het is met opzet dezelfde resolutie
    /// en geen nagebouwde voorwaarde ernaast: wat een besluit nodig heeft, staat
    /// in zijn definitie, en een tweede opsomming zou ervan gaan afwijken zodra
    /// er een input bij komt.
    ///
    /// **Alleen de eigen kronieken.** [`BesluitInput::FromChronicle`] en
    /// [`BesluitInput::FromDecretogram`] leest de cel bij zichzelf, dus die zijn
    /// hier zonder meer na te lopen. [`BesluitInput::AcceptFrom`] niet: die vergt
    /// een vraag over een celgrens, en een beschikbaarheidscheck mag geen contact
    /// zijn — anders zou het openslaan van een scherm verkeer opleveren dat
    /// niemand vroeg (invariant I1). Of de ander iets vaststelde, blijft dus een
    /// zaak van het besluit zelf. [`BesluitInput::Param`] evenmin: dat is wat de
    /// invuller invult, en een leeg formulierveld is geen ontbrekend feit.
    ///
    /// Er komt **niets** naar buiten behalve de reden: geen waarde, geen gram.
    /// Dit leest niet anders dan wat het beeld van de wereld toch al over deze
    /// cel laat zien.
    ///
    /// De eerste die ontbreekt en niet alle, want dit is een reden voor een
    /// lezer en geen foutenlijst: wie het eerste feit aanlevert, krijgt vanzelf
    /// het volgende te zien.
    pub(crate) fn missing_own_fact(
        &self,
        besluit: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Option<String> {
        // Onbekend besluit: het optuigen heeft elke `decides`-actie aan een
        // bestaande definitie gebonden, dus hier is niets te melden.
        let definition = self.definition(besluit).ok()?;
        let zaakkenmerk = definition.zaakkenmerk(&self.id, params);
        let gaps = definition.zaakkenmerk_gaps(params);
        for (input, origin) in &definition.inputs {
            let gathered = match origin {
                BesluitInput::FromChronicle { chronicle, field } => {
                    self.read_own_chronicle(&definition, input, chronicle, field, params, op_moment)
                }
                BesluitInput::FromDecretogram {
                    besluit: earlier,
                    field,
                } => match &zaakkenmerk {
                    // Zonder de parameters waarop de zaak gevonden wordt, is er
                    // geen zaak — en dan is "geen eerder besluit voor zaak ''"
                    // een melding waarin een lezer een kenmerk zoekt dat er niet
                    // is. Zeg dan wat er nog ontbreekt.
                    Ok(_) if !gaps.is_empty() => Err(SimulatorError::BesluitInputMissing {
                        cell: self.id.clone(),
                        besluit: definition.name.clone(),
                        input: input.clone(),
                        reason: format!(
                            "de vraag noemt nog geen {}, dus er is geen zaak aan te \
                             wijzen waarin een eerder besluit '{earlier}' zou liggen",
                            gaps.join(" en ")
                        ),
                    }),
                    Ok(zaakkenmerk) => self.read_earlier_decretogram(
                        &definition,
                        input,
                        earlier,
                        field,
                        zaakkenmerk,
                        op_moment,
                    ),
                    // Een kenmerk dat niet eenduidig in te vullen is, houdt het
                    // besluit tegen zodra het genomen wordt; dan hoort het de
                    // actie nu ook tegen te houden, met diezelfde melding.
                    Err(error) => return Some(error.to_string()),
                },
                BesluitInput::Param { .. } | BesluitInput::AcceptFrom { .. } => continue,
            };
            if let Err(error) = gathered {
                return Some(error.to_string());
            }
        }
        None
    }

    /// Voer de regeling uit op het moment van het besluit en maak het decretogram.
    ///
    /// De verzamelde inputs gaan als engine-parameters mee: een waarde onder de
    /// naam van een input vervangt daar haar `source`, dus het besluit leunt
    /// aantoonbaar op precies de feiten die het verzamelde. De rest van wat de
    /// regeling nodig heeft, komt uit de eigen kronieken als databron — dezelfde
    /// weg als bij een reductie, want dat is nog altijd tier 1.
    ///
    /// `competent_authority` is al opgelost — in [`Self::decide`], want daar
    /// wordt er ook op geweigerd. Het hier nóg eens opzoeken zou twee antwoorden
    /// op dezelfde vraag mogelijk maken, en dan kon een gram een ander gezag
    /// dragen dan het gezag waarop het is toegelaten.
    fn execute(
        &self,
        definition: &BesluitDefinition,
        context: DecisionContext<'_>,
        draft: DecisionDraft<'_>,
        resolver: Option<Rc<dyn CellResolver>>,
    ) -> Result<(Decretogram, BTreeMap<String, Value>)> {
        let DecisionContext {
            identity,
            op_moment,
            ..
        } = context;
        let DecisionDraft {
            zaakkenmerk,
            inputs,
            authority: competent_authority,
            declared,
        } = draft;
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
        self.grant_cell_tier(&mut service, resolver)?;
        // De stand van elke stroom die zojuist klaargezet is, voor in het gram
        // (RFC-022 §1.3). Hier en niet in een reductie: die legt niets vast en
        // hoort er dus ook niet voor te betalen.
        let chronicle_sources: Vec<ChronicleSource> = self
            .chronicles
            .stand(op_moment)?
            .into_iter()
            .filter(|stand| stand.stream != BESCHIKKINGEN)
            .map(|stand| ChronicleSource {
                chronicle: stand.stream,
                op_moment,
                grams: stand.grams,
                content_hash: stand.content_hash,
            })
            .collect();

        let engine_params: BTreeMap<String, Value> = inputs
            .iter()
            .map(|(name, input)| (name.clone(), input.value.clone()))
            .collect();
        let calculation_date = op_moment.format("%Y-%m-%d").to_string();

        // Wat het uitvoerende artikel over dít besluit zegt, gelezen vóór de
        // uitvoering: het rechtskarakter, het besluittype en de voorwaarden
        // waaronder dit besluit een afwijzing is. Vóór en niet erna, want de
        // voorwaarden bepalen mede wát er uitgerekend moet worden — zie
        // `requested` hieronder. Uit de versie die op dit moment gold: wat de wet
        // zegt, zegt ze in het recht van toen.
        let (legal_character, declared_decision_type, conditions) = {
            let resolver = service.resolver();
            // Onbereikbaar fout: het optuigen heeft dit blok voor elke geladen
            // versie al gelezen — met deze lezer, zodat de twee niet uiteen
            // kunnen lopen. Stil doorgaan zou hier van een weigering een
            // toekenning maken, en dat mag nooit aan een aanname hangen.
            let (_, origin, produces) = executing_article(
                resolver,
                &definition.regulation,
                &definition.output,
                op_moment,
            );
            let conditions = ChronolexBlock::read(produces, &origin)?
                .afwijzing_wanneer
                .as_ref()
                .map(extensions::afwijzing_wanneer)
                .transpose()
                .map_err(|reason| SimulatorError::MalformedAfwijzingWanneer {
                    cell: self.id.clone(),
                    besluit: definition.name.clone(),
                    regulation: definition.regulation.clone(),
                    output: definition.output.clone(),
                    reason,
                })?
                .unwrap_or_default();
            (
                produces.and_then(|produces| produces.legal_character.clone()),
                produces.and_then(|produces| produces.decision_type.clone()),
                conditions,
            )
        };
        // En het moet een beschikking zijn: een decretogram ís een uitkomst met
        // `legal_character: BESCHIKKING` (RFC-022 §1.2). Het optuigen heeft dat
        // voor elke geladen versie al getoetst; hier nog eens, op de versie die op
        // dit moment geldt, zodat een gram nooit iets anders draagt dan wat het is.
        //
        // Het rechtskarakter hoort bij het artikel dat de aansturende uitkomst
        // voortbrengt: díe uitkomst *is* het besluit. Een uitkomst die erbij
        // meegaat kan uit een ander artikel komen, en dat artikel zegt niets over
        // het karakter van dit besluit.
        let legal_character = match legal_character {
            Some(character) if character == BESCHIKKING => character,
            other => {
                return Err(SimulatorError::BesluitNotABeschikking {
                    cell: self.id.clone(),
                    besluit: definition.name.clone(),
                    regulation: definition.regulation.clone(),
                    output: definition.output.clone(),
                    found: other.unwrap_or_else(|| "geen `legal_character`".to_string()),
                })
            }
        };

        // De uitkomsten die het gram draagt, plus de uitkomsten waarop de wet
        // haar afwijzing laat afhangen, plus de bedragen waarover de
        // verplichtingen van dit artikel gaan. Die laatste twee hoeft de
        // definitie niet vast te leggen — de grond komt als `afwijzingsgrond` in
        // het gram en het bedrag als uitgerekend *schema* — maar uitgerekend
        // moeten ze worden: anders raakt de voorwaarde op een ontbrekende waarde
        // stil nooit vervuld, en hangt het bedrag af van wat de uitvoering
        // toevallig ook uitrekende.
        let requested_outputs: BTreeSet<&str> = definition
            .recorded_outputs()
            .into_iter()
            .chain(conditions.keys().map(String::as_str))
            .chain(declared.amounts())
            .chain(declared.schedule_outputs(definition))
            .collect();
        let recorded: Vec<&str> = requested_outputs.into_iter().collect();
        // Mét trace, en dat is het verschil met een reductie: het decretogram
        // draagt het RFC-013 receipt van deze uitvoering, en zonder de trace
        // staat er wel wát er uitkwam maar niet langs welke artikelen. Dan is
        // het besluit na te rekenen maar niet na te lopen.
        //
        // `new_untimed` en niet `new`: een getimede trace zet op elke node hoe
        // lang die stap duurde, en dat is per run anders. Dat mag niet in een
        // gram — een hash over hetzelfde besluit met dezelfde invoer hoort
        // tweemaal hetzelfde te zijn — en het is voor reproduceerbaarheid ook
        // niets waard. Het scheelt bovendien twee `Instant::now()` per node.
        let result = service
            .evaluate_law_with_trace_builder(
                &definition.regulation,
                &recorded,
                engine_params.clone(),
                &calculation_date,
                TraceBuilder::new_untimed(),
            )
            .map_err(|error| self.explain_undeclared_source(definition, untraced(error)))?;

        let requested: Vec<String> = recorded.iter().map(|name| (*name).to_string()).collect();
        let receipt = service.build_receipt_with_outputs(
            &result,
            &engine_params,
            &calculation_date,
            &requested,
        );

        let resolver = service.resolver();
        // Welke regelingen deze uitvoering werkelijk uitvoerde, in de versie die
        // op dit moment gold. Hier en niet bij het lezen van het gram: alleen
        // tijdens de uitvoering is bekend welke regeling welke input leverde.
        let executed_regulations = executed_regulations(
            &definition.regulation,
            result.regulation_valid_from.as_deref(),
            &result,
            resolver,
            op_moment,
        );
        let outputs: BTreeMap<String, Value> = definition
            .recorded_outputs()
            .into_iter()
            .filter_map(|name| {
                result
                    .outputs
                    .get(name)
                    .map(|value| (name.to_string(), value.clone()))
            })
            .collect();
        // Ketste het besluit af op een voorwaarde die de **wet** noemt? Dan is het
        // een afwijzing: hetzelfde gram, hetzelfde rechtskarakter, maar een ander
        // besluittype en straks geen verplichtingen (Awb 1:3 lid 2 — een
        // beschikking omvat ook de afwijzing van de aanvraag).
        //
        // De voorwaarden komen van hetzelfde `produces` als het rechtskarakter
        // hierboven, en dus uit de versie die op dit moment gold: wat de wet zegt,
        // zegt ze in het recht van toen. Het artikel bij elke grond wordt er apart
        // bij gezocht — een voorwaarde mag over een uitkomst van een ander artikel
        // gaan, en dan is dát de grondslag van de weigering.
        //
        // Getoetst tegen wat de uitvoering opleverde en niet tegen wat het gram
        // draagt: een voorwaarde hoeft geen uitkomst te zijn die de definitie
        // vastlegt (ze is hierboven juist daarom bij `recorded` gezet), en dan
        // zou ze op de uitkomsten van het gram stil nooit vervuld raken.
        let afwijzingsgronden =
            besluit::afwijzingsgronden(&conditions, &result.outputs, |output| {
                resolver
                    .get_article_by_output(&definition.regulation, output, Some(op_moment))
                    .map(|article| article.number.clone())
            });
        // Het type van de regeling zolang er niets afketst, en anders dat van de
        // afwijzing. Andersom zou een uitvoeringsregel die "TOEKENNING" zegt een
        // weigering als toekenning laten vastleggen.
        let decision_type = if afwijzingsgronden.is_empty() {
            declared_decision_type
        } else {
            Some(besluit::AFWIJZING.to_string())
        };

        Ok((
            Decretogram {
                cell: self.id.clone(),
                besluit: definition.name.clone(),
                zaakkenmerk,
                op_moment,
                regulation: definition.regulation.clone(),
                regulation_valid_from: result.regulation_valid_from.clone(),
                competent_authority,
                besloten_door: identity.to_string(),
                legal_character,
                decision_type,
                afwijzingsgronden,
                executed_regulations,
                chronicle_sources,
                outputs,
                inputs,
                // Het schema komt er in `decide` bij: het hangt aan de uitkomsten
                // hierboven, en die zijn hier net pas bekend. Dat geldt voor
                // beide helften — wat nu vervalt en wat op de bekendmaking wacht.
                obligations: Vec::new(),
                wacht_op_bekendmaking: Vec::new(),
                niets_te_betalen: Vec::new(),
                hooks_niet_uitgevoerd: hooks_niet_uitgevoerd(
                    resolver,
                    &result.skipped_hooks,
                    op_moment,
                ),
                receipt,
            },
            // Álles wat de uitvoering opleverde, en niet alleen wat het gram
            // vastlegt: het bedrag van een verplichting mag een uitkomst van het
            // artikel zijn die dit besluit niet publiceert.
            result.outputs,
        ))
    }

    /// Kom één vervallen verplichting na: leg de betaling vast.
    ///
    /// Dit is wat de cel die de verplichting draagt doet als de klok een
    /// vervaldatum passeert. Ze legt vast wat zíj deed — een executogram met
    /// `intake: betaling` in haar eigen stroom [`BETALINGEN`] — en niets over een
    /// ander.
    ///
    /// Levert `false` als deze termijn er al lag: zie [`Self::already_settled`].
    pub(crate) fn pay_obligation(&mut self, due: &ObligationDue, betaler: &str) -> Result<bool> {
        self.record_obligation(due, due.payment_event(betaler))
    }

    /// Leg vast dat gemeld is dat er op een verplichting betaald is.
    ///
    /// De tegenhanger van [`Self::pay_obligation`], bij de cel die besloot. Ook
    /// dit is een eigen vastlegging (`intake: levering`) en geen kopie van het
    /// gram van de betaler: beide kanten weten wat er gebeurde, en geen van
    /// beide leest de kroniek van de ander.
    pub(crate) fn note_obligation_paid(&mut self, due: &ObligationDue) -> Result<bool> {
        self.record_obligation(due, due.delivery_event())
    }

    /// Leg één kant van een vervallen termijn vast, tenzij ze er al ligt.
    ///
    /// Langs [`Self::record`] en niet rechtstreeks naar de store: dat is de poort
    /// waar elke vastlegging langs hoort, ook een die het platform zelf maakt.
    fn record_obligation(&mut self, due: &ObligationDue, event: ChronicleEvent) -> Result<bool> {
        if self.already_settled(due) {
            return Ok(false);
        }
        self.record(BETALINGEN, event)?;
        Ok(true)
    }

    /// Ligt deze termijn hier al?
    ///
    /// Eén zaak, één besluit en één volgnummer is één termijn. Dat is wat een
    /// executogram twee keer vastleggen tegenhoudt: de klok mag in kleine stappen
    /// langskomen, een besluit mag overgedaan worden, en een wereld mag de
    /// betaling al als startstand hebben staan mits die dezelfde verwijzing naar
    /// het besluit draagt — betaald is betaald, en een kroniek die hetzelfde feit
    /// twee keer draagt telt het in een som ook twee keer mee.
    ///
    /// Het besluit hoort in die sleutel en niet alleen het volgnummer: over één
    /// zaak worden meer besluiten genomen (een verlening en later een
    /// vaststelling), en die dragen elk hun eigen schema dat bij 1 begint. Op
    /// alleen zaak en volgnummer zou de termijn van het tweede besluit voor die
    /// van het eerste doorgaan en stil wegvallen. Dat het volgnummer bínnen een
    /// besluit uniek is, komt van de andere kant — zie
    /// [`ObligationDue::volgnummer`].
    ///
    /// Het *moment* van het besluit zit er met opzet niet in. Hetzelfde besluit
    /// over dezelfde zaak nog eens nemen levert daardoor geen tweede betaling,
    /// ook niet op een latere dag. Een herzieningsbesluit dat een eerder schema
    /// vervángt, is iets anders — dat vraagt om intrekken, en intrekken bestaat
    /// hier nog niet.
    pub(crate) fn already_settled(&self, due: &ObligationDue) -> bool {
        self.chronicles
            .latest_recording(
                BETALINGEN,
                besluit::ZAAKKENMERK,
                &Value::String(due.zaakkenmerk.clone()),
                &BTreeMap::from([
                    (besluit::VOLGNUMMER.to_string(), Value::Int(due.volgnummer)),
                    (
                        besluit::BESLUIT.to_string(),
                        Value::String(due.besluit.clone()),
                    ),
                    (
                        besluit::BESLUIT_CEL.to_string(),
                        Value::String(due.decided_by.clone()),
                    ),
                ]),
                due.vervaldatum,
            )
            .is_some()
    }

    /// Het sleutelveld van een stroom die deze cel houdt; `None` als ze haar
    /// niet houdt.
    ///
    /// `pub(crate)`, en alleen voor het optuigen: de wereld moet kunnen toetsen
    /// dat een cel die een verplichting draagt een betalingsstroom heeft met de
    /// sleutel waarop een zaak terug te vinden is. Geeft niets prijs over wat er
    /// in die stroom staat.
    pub(crate) fn stream_key(&self, stream: &str) -> Option<&str> {
        self.chronicles.key_of(stream)
    }
}

/// Eén tekstveld van een gram, of leeg als het gram het niet draagt.
///
/// Voor de dagelijkse vragen over de eigen stroom [`BESCHIKKINGEN`]: welke stage,
/// welk besluit, welke zaak. Leeg in plaats van `None`, want de aanroeper
/// vergelijkt: een gram zonder dat veld is een gram dat het gevraagde niet is.
fn gram_tekst<'a>(event: &'a ChronicleEvent, name: &str) -> &'a str {
    event
        .fields
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// De eigenlijke fout uit een uitvoering met trace.
///
/// Mislukt een getraceerde uitvoering, dan levert de engine de fout verpakt in
/// `EngineError::TracedError`, met de halve trace erbij. Dat is bruikbaar voor
/// wie de trace wil zien, maar het verstopt de fout voor wie erop wil kijken —
/// en dat doen [`Cell::explain_undeclared_source`] en de weigeringen die eraan
/// hangen wél. Hier gaat de verpakking eraf, zodat het aanzetten van de trace
/// geen enkele foutmelding verandert.
///
/// In een lus, want een geneste uitvoering kan er nog een laag omheen zetten.
fn untraced(error: EngineError) -> EngineError {
    let mut error = error;
    while let EngineError::TracedError { source, .. } = error {
        error = *source;
    }
    error
}

/// Het bevoegd gezag zoals een regelingversie het declareert (RFC-002), tot een
/// naam gebracht.
///
/// Twee vormen in het schema, en een derde die eruitziet als de eerste: een
/// naam die met `#` begint is een **verwijzing** naar een uitkomst van de
/// regeling zelf (`competent_authority: '#bevoegd_gezag'`). Die uitkomst is niet
/// altijd als `output` gedeclareerd — vaak zet één actie haar rechtstreeks — dus
/// ze is niet via de engine op te vragen; het geladen law-model is de plek waar
/// ze wél staat. De verwijzing wordt in de hele regeling opgezocht, of de
/// declaratie nu op het artikel of op het document staat.
///
/// Wat er met de uitkomst gebeurt, staat in [`Cell::check_competent_authority`]:
/// de cel die besluit moet deze naam dragen, en anders is er geen besluit.
fn competent_authority(law: &ArticleBasedLaw, declared: &CompetentAuthority) -> Option<String> {
    match declared {
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

/// Het artikel dat een uitkomst van een regeling voortbrengt, in de versie die op
/// `moment` gold, met zijn herkomst en zijn `produces`.
///
/// Eén opzoeking voor het besluit en de stage erna: allebei lezen ze het
/// `chronolex`-blok van dít artikel, en twee eigen opzoekingen zouden bij een
/// wetswijziging uiteen kunnen lopen.
fn executing_article<'a>(
    resolver: &'a RuleResolver,
    regulation: &str,
    output: &str,
    moment: NaiveDate,
) -> (
    Option<&'a Article>,
    ObligationOrigin,
    Option<&'a regelrecht_engine::article::Produces>,
) {
    let article = resolver.get_article_by_output(regulation, output, Some(moment));
    let produces = article
        .and_then(Article::get_execution_spec)
        .and_then(|execution| execution.produces.as_ref());
    let origin = ObligationOrigin {
        regulation: regulation.to_string(),
        valid_from: resolver
            .get_law_for_date(regulation, Some(moment))
            .and_then(|law| law.valid_from.clone()),
        article: article.map_or_else(String::new, |article| article.number.clone()),
    };
    (article, origin, produces)
}

/// De hooks die de engine oversloeg, met de versie van hun regeling erbij.
fn hooks_niet_uitgevoerd(
    resolver: &RuleResolver,
    skipped: &[SkippedHook],
    moment: NaiveDate,
) -> Vec<HookNietUitgevoerd> {
    skipped
        .iter()
        .map(|hook| HookNietUitgevoerd {
            lexogram: ObligationOrigin {
                regulation: hook.law_id.clone(),
                valid_from: resolver
                    .get_law_for_date(&hook.law_id, Some(moment))
                    .and_then(|law| law.valid_from.clone()),
                article: hook.article.clone(),
            },
            hook_point: hook.hook_point.clone(),
            ontbrekende_input: hook.missing_input.clone(),
        })
        .collect()
}

/// Twee namen vergelijkbaar maken: zonder witruimte eromheen, zonder kasus.
///
/// Meer niet. Een organisatienaam is tekst die een mens intikt, dus `' Dienst
/// Toeslagen'` en `'dienst toeslagen'` zijn dezelfde organisatie. Wat er wél
/// verschil in maakt — een afkorting, een oude naam, een afdeling erbij — blijft
/// verschil: dat is een vraag over wie er bevoegd is, en die hoort in de wet of
/// in het wereldbestand beantwoord te worden en niet hier geraden.
pub(crate) fn normalised(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Waar een kroniekfilter naar kijkt: één stroom, één onderwerp, één filter.
///
/// De twee reductievormen over een kroniek — de laatste vastlegging en de som —
/// stellen dezelfde vraag en verschillen alleen in wat ze met het antwoord doen.
/// Dat ze hier dezelfde vraag dragen, houdt ze bij elkaar: een filter dat voor de
/// som iets anders zou betekenen dan voor `latest`, zou twee reducties opleveren
/// die niet over dezelfde vastleggingen gaan.
struct ChronicleQuery<'a> {
    /// De eigen stroom waarover gefilterd wordt.
    chronicle: &'a str,
    /// Het sleutelveld van die stroom.
    key: &'a str,
    /// De sleutelwaarde uit de vraag.
    key_value: &'a Value,
    /// De extra gelijkheidsvoorwaarden (`where`).
    conditions: &'a BTreeMap<String, Value>,
}

impl ChronicleQuery<'_> {
    /// Dit filter zoals het in de uitleg bij het antwoord komt te staan.
    fn describe(&self, regel: Regel, op_moment: NaiveDate) -> Kroniekfilter {
        Kroniekfilter {
            chronicle: self.chronicle.to_string(),
            key: self.key.to_string(),
            key_value: self.key_value.clone(),
            conditions: self.conditions.clone(),
            regel,
            op_moment,
        }
    }

    /// Wat er in de stroom lag en toch niet meedeed.
    fn missed(&self, cell: &Cell, op_moment: NaiveDate) -> Gemist {
        cell.chronicles.missed(
            self.chronicle,
            self.key,
            self.key_value,
            self.conditions,
            op_moment,
        )
    }

    /// Waarom dit filter niets vond, zo precies dat het na te lopen is.
    fn nothing_established(&self, op_moment: NaiveDate) -> String {
        nothing_established(
            self.chronicle,
            self.key,
            self.key_value,
            self.conditions,
            op_moment,
        )
    }
}

/// Waarom het kroniekfilter niets vond, zo precies dat het na te lopen is.
///
/// Los van [`ChronicleQuery`], omdat het besluit-pad dezelfde reden nodig heeft
/// voor een input die het niet kon ophalen, en daar is geen lexostatus in zicht.
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
    query: &ChronicleQuery<'_>,
    recorded: NaiveDate,
) -> String {
    let published = definition
        .published_outputs()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "kroniekstroom '{}' heeft voor {} '{}' wel een vastlegging (van {recorded}), \
         maar die draagt geen van de gepubliceerde uitkomsten ({published})",
        query.chronicle, query.key, query.key_value
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

/// Controleer de cel-bronnen van een configuratie en zet ze op sleutel.
///
/// Drie weigeringen, alle drie bij het optuigen en niet pas bij het eerste
/// besluit:
///
/// - een bron die **geen van de eigen wetten noemt**. Dan zou de cel een peer
///   mogen bevragen zonder dat haar recht daar ooit om vraagt, en dat is precies
///   het vraaggraf dat invariant I3 uitsluit;
/// - een cel-id dat **een eigen regeling overschaduwt**. De engine weigert dat
///   ook (RFC-022 §4.2), maar pas bij het registreren van de resolver: een vraag
///   voor de cel zou anders door de gelijknamige regeling beantwoord worden;
/// - **twee afspraken over dezelfde `(cel, uitkomst)`**, want dan beslist de
///   volgorde in het bestand welke lexostatus gevraagd wordt.
///
/// Geeft naast de afspraken op sleutel ook de namen terug die de wetten aanwijzen
/// en die de cel niet laadt. Die lijst is wat een engine-melding over een
/// "onbekende regeling" van een verdwaalde naam kan onderscheiden.
fn check_accepted_sources(
    config: &CellConfig,
    service: Option<&LawExecutionService>,
) -> Result<CellSources> {
    let asked = service
        .map(|service| cell_sources_in_laws(service, &config.laws))
        .unwrap_or_default();

    let mut sources: BTreeMap<(String, String), AcceptedSource> = BTreeMap::new();
    for source in &config.accepts_from {
        if config.laws.iter().any(|law| law == &source.cell) {
            return Err(SimulatorError::CellShadowsRegulation {
                cell: config.id.clone(),
                peer: source.cell.clone(),
            });
        }
        let key = (source.cell.clone(), source.output.clone());
        if !asked.contains(&key) {
            return Err(SimulatorError::UnclaimedCellSource {
                cell: config.id.clone(),
                peer: source.cell.clone(),
                output: source.output.clone(),
                asked: asked
                    .iter()
                    .map(|(cell, output)| format!("{cell}.{output}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }
        if sources.insert(key, source.clone()).is_some() {
            return Err(SimulatorError::DuplicateCellSource {
                cell: config.id.clone(),
                peer: source.cell.clone(),
                output: source.output.clone(),
            });
        }
    }

    Ok(CellSources {
        foreign: asked.into_iter().map(|(cell, _)| cell).collect(),
        declared: sources,
    })
}

/// Wat de wetten van een cel buiten zichzelf aanwijzen, en wat daarover is
/// afgesproken.
///
/// De twee horen bij elkaar: de afspraken zijn een deelverzameling van wat de
/// wetten vragen, en alleen samen kunnen ze een engine-melding over een onbekende
/// naam plaatsen.
struct CellSources {
    /// De afspraken uit `accepts_from`, op `(cel, uitkomst)` zoals de engine
    /// ernaar vraagt.
    declared: BTreeMap<(String, String), AcceptedSource>,
    /// Elke naam die de wetten in een `source.regulation` noemen en die de cel
    /// niet zelf laadt — met en zonder afspraak.
    foreign: BTreeSet<String>,
}

/// De `(cel, uitkomst)`-paren die de wetten van deze cel bij een **andere cel**
/// halen: elke `source.regulation` die geen regeling is die de cel zelf laadt.
///
/// Dat is dezelfde lezing als de engine hanteert (RFC-022 §4.2): een
/// `source.regulation` wijst een producent aan, en of dat een regeling of een
/// organisatie is, blijkt uit wat er geladen is. De uitkomstnaam is
/// `source.output`, of — als de wet die niet noemt — de naam van de input zelf,
/// precies wat de engine aan een resolver doorgeeft.
fn cell_sources_in_laws(
    service: &LawExecutionService,
    own_laws: &[String],
) -> BTreeSet<(String, String)> {
    let mut sources = BTreeSet::new();
    for law in service.resolver().all_law_versions() {
        for article in &law.articles {
            let Some(execution) = article.get_execution_spec() else {
                continue;
            };
            for input in execution.input.iter().flatten() {
                let Some(source) = input.source.as_ref() else {
                    continue;
                };
                let Some(regulation) = source.regulation.as_ref() else {
                    continue;
                };
                if own_laws.iter().any(|law| law == regulation) {
                    continue;
                }
                let output = source.output.clone().unwrap_or_else(|| input.name.clone());
                sources.insert((regulation.clone(), output));
            }
        }
    }
    sources
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

/// Per regeling en per uitkomst het rechtskarakter dat het voortbrengende
/// artikel eraan geeft, over alle geladen versies heen.
///
/// `None` in de verzameling betekent dat een versie het artikel wél kent maar
/// er geen `produces.legal_character` op zet. Alle versies tellen mee, om
/// dezelfde reden als bij [`outputs_per_regulation`]: een besluit over een ouder
/// moment landt op een oudere versie, en een decretogram hoort onder élke versie
/// een beschikking te zijn (RFC-022 §1.2).
fn legal_characters_per_output(
    service: &LawExecutionService,
) -> BTreeMap<String, BTreeMap<String, BTreeSet<Option<String>>>> {
    let mut per_regulation: BTreeMap<String, BTreeMap<String, BTreeSet<Option<String>>>> =
        BTreeMap::new();
    for law in service.resolver().all_law_versions() {
        let known = per_regulation.entry(law.id.clone()).or_default();
        for article in &law.articles {
            let Some(execution) = article.get_execution_spec() else {
                continue;
            };
            let character = execution
                .produces
                .as_ref()
                .and_then(|produces| produces.legal_character.clone());
            for output in execution.output.iter().flatten() {
                known
                    .entry(output.name.clone())
                    .or_default()
                    .insert(character.clone());
            }
        }
    }
    per_regulation
}

/// Per regeling en per uitkomst het type dat de geladen versies haar geven, bij
/// de naam die een wetsbestand ervoor schrijft.
///
/// Naast [`legal_characters_per_output`] en met dezelfde vorm: alle versies
/// tellen mee, want een besluit over een ouder moment landt op een oudere
/// versie. Hiermee is `afwijzing_wanneer` bij het optuigen na te lopen — een
/// voorwaarde vergelijkt met `true` of `false`, dus de uitkomst moet onder elke
/// versie een ja-of-nee zijn.
fn types_per_output(
    service: &LawExecutionService,
) -> BTreeMap<String, BTreeMap<String, BTreeSet<String>>> {
    let mut per_regulation: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    for law in service.resolver().all_law_versions() {
        let known = per_regulation.entry(law.id.clone()).or_default();
        for article in &law.articles {
            let Some(execution) = article.get_execution_spec() else {
                continue;
            };
            for output in execution.output.iter().flatten() {
                known
                    .entry(output.name.clone())
                    .or_default()
                    .insert(output_type_name(output.output_type).to_string());
            }
        }
    }
    per_regulation
}

/// De naam die een wetsbestand aan het type van een uitkomst geeft.
///
/// De naam uit het schema en niet die van de Rust-variant: wat in een
/// foutmelding over `afwijzing_wanneer` staat, hoort het woord te zijn dat in het
/// wetsbestand staat. Eén plek, zodat de toets ([`config::BOOLEAN`]) en de
/// melding hetzelfde woord lezen.
fn output_type_name(value_type: regelrecht_engine::ParameterType) -> &'static str {
    use regelrecht_engine::ParameterType as Type;
    match value_type {
        Type::String => "string",
        Type::Number => "number",
        Type::Boolean => config::BOOLEAN,
        Type::Amount => "amount",
        Type::Date => "date",
        Type::Array => "array",
        Type::Object => "object",
    }
}

/// Het bevoegd gezag dat een versie voor dit artikel declareert (RFC-002).
///
/// Het **artikel** gaat voor het document: één wet kan nul tot veel bevoegde
/// gezagen kennen, en het artikel dat de uitkomst voortbrengt zegt wie daarover
/// mag besluiten. Het documentniveau is de terugvaloptie — het schema laat het
/// toe en het corpus gebruikt het.
///
/// Eén plek voor die voorrangsregel, want twee vragen leunen erop: wie mag
/// besluiten, en namens wie moet er betaald worden. Zouden ze uiteenlopen, dan
/// kon een besluit onder het ene gezag genomen worden en onder het andere
/// betaald.
fn declared_authority<'a>(
    law: &'a ArticleBasedLaw,
    article: Option<&'a Article>,
) -> Option<&'a CompetentAuthority> {
    article
        .and_then(|article| article.machine_readable.as_ref())
        .and_then(|machine_readable| machine_readable.competent_authority.as_ref())
        .or(law.competent_authority.as_ref())
}

/// Wat de geladen versies onder `chronolex` declareren, per regeling en per
/// uitkomst.
///
/// Twee kaarten uit **één** lezing, en dat is de hele reden dat ze samen in één
/// functie staan: de verplichtingen en de afwijzingsvoorwaarden komen uit
/// hetzelfde blok, en zouden ze elk hun eigen weg naar `produces.extensions`
/// zoeken, dan kan er één zijn die een blok ziet dat de ander overslaat. Dan
/// glipt een regeling die denkt te weigeren langs de plek die dat had moeten
/// weigeren.
///
/// Alleen de artikelen die het blok dragen: de rest declareert niets, en een
/// ingang zonder inhoud zou de kaart laten groeien met het corpus in plaats van
/// met wat de wereld gebruikt. Alle versies tellen mee, om dezelfde reden als
/// bij [`legal_characters_per_output`] — een besluit over een ouder moment landt
/// op een oudere versie, en wat daar staat hoort net zo goed te kloppen.
struct DeclaredChronolex {
    /// Per regeling en per uitkomst wat de versies eraan opleggen.
    obligations: ObligationsPerOutput,
    /// Per regeling en per uitkomst van het artikel de `stage_uitkomsten` voor
    /// de stage BEKENDMAKING, over alle versies samen.
    stage_uitkomsten: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    /// Per regeling en per uitkomst het `afwijzing_wanneer`-blok, ongelezen.
    ///
    /// Ongelezen omdat de melding erover de cel en het besluit noemt, en die
    /// weet deze kaart niet: het uitpakken gebeurt bij het optuigen van de
    /// besluit-definitie (zie `BesluitDefinition::validate`).
    afwijzing_blocks: BTreeMap<String, BTreeMap<String, Vec<Value>>>,
}

impl DeclaredChronolex {
    /// Een cel zonder wetten: er valt niets te lezen.
    fn empty() -> Self {
        Self {
            obligations: ObligationsPerOutput::new(),
            stage_uitkomsten: BTreeMap::new(),
            afwijzing_blocks: BTreeMap::new(),
        }
    }
}

/// Lees het `chronolex`-blok van elk artikel van elke geladen versie.
///
/// Hier valt de weigering op een blok dat niet te lezen is: bij het **optuigen**
/// van de cel, en niet bij de eerste aanvrager. Het blok staat in een wet, dus
/// wie het schrijft is niet dezelfde als wie het leest, en een typfout die stil
/// eindigt levert een regeling op die denkt te verplichten of te weigeren en
/// niets doet.
///
/// Een gezag dat niet op te lossen is (een `#`-verwijzing die nergens op
/// uitkomt) telt hier als "geen gezag". De weigering daarvoor staat bij het
/// besluit zelf ([`Cell::competent_authority`]), met het gezag in de melding;
/// hier zou ze de wereld laten omvallen op een artikel dat niemand uitvoert.
fn chronolex_per_output(
    service: &LawExecutionService,
    hook_outputs: &BTreeSet<String>,
) -> Result<DeclaredChronolex> {
    let mut declared_chronolex = DeclaredChronolex::empty();
    for law in service.resolver().all_law_versions() {
        let law_outputs: BTreeSet<String> = law
            .articles
            .iter()
            .filter_map(Article::get_execution_spec)
            .flat_map(|execution| execution.output.iter().flatten())
            .map(|output| output.name.clone())
            .collect();
        for article in &law.articles {
            let Some(execution) = article.get_execution_spec() else {
                continue;
            };
            if !ChronolexBlock::declared_in(execution.produces.as_ref()) {
                continue;
            }
            let origin = ObligationOrigin {
                regulation: law.id.clone(),
                valid_from: law.valid_from.clone(),
                article: article.number.clone(),
            };
            // Het ene parse-punt. Wat hieronder in de twee kaarten belandt, komt
            // uit deze ene lezing en nergens anders vandaan.
            let mut block = ChronolexBlock::read(execution.produces.as_ref(), &origin)?;
            block.check_stage_uitkomsten(&origin, &law_outputs)?;
            let eigen: BTreeSet<String> = block
                .stage_uitkomsten_voor(besluit::STAGE_BEKENDMAKING)
                .iter()
                .cloned()
                .collect();
            // Uit het blok gehaald en niet gekopieerd: de verplichtingenkaart
            // heeft alleen `verplichtingen` nodig, en de voorwaarden gaan hier
            // hun eigen kaart in.
            let afwijzing = block.afwijzing_wanneer.take();
            let declared = DeclaredObligations::from_block(
                origin,
                declared_authority(law, Some(article))
                    .and_then(|declared| competent_authority(law, declared)),
                article,
                block,
                hook_outputs,
            );
            let per_output = declared_chronolex
                .stage_uitkomsten
                .entry(law.id.clone())
                .or_default();
            for output in execution.output.iter().flatten() {
                per_output
                    .entry(output.name.clone())
                    .or_default()
                    .extend(eigen.iter().cloned());
            }
            let obligations = declared_chronolex
                .obligations
                .entry(law.id.clone())
                .or_default();
            let blocks = declared_chronolex
                .afwijzing_blocks
                .entry(law.id.clone())
                .or_default();
            for output in execution.output.iter().flatten() {
                obligations
                    .entry(output.name.clone())
                    .or_default()
                    .push(declared.clone());
                if let Some(block) = afwijzing.as_ref() {
                    blocks
                        .entry(output.name.clone())
                        .or_default()
                        .push(block.clone());
                }
            }
        }
    }
    Ok(declared_chronolex)
}

/// De uitkomsten die de hooks op de stage BEKENDMAKING kunnen opleveren.
///
/// Bekend vóór de eerste bekendmaking, om dezelfde reden als bij
/// [`besluit::declared_fields`]: een lexostatus die over `beschikkingen`
/// reduceert wordt bij het optuigen getoetst, en dan is er nog geen enkel gram.
/// Zonder deze lijst zou een vraag naar de uiterste betaaldatum als typfout
/// geweigerd worden — terwijl in de wet gewoon te lezen staat dat die uitkomst
/// er bij die stage uit komt.
///
/// Uit de **wet** en niet uit een lijst hier: welk artikel op welke stage vuurt,
/// staat in het lexogram (RFC-007/RFC-008). Over alle geladen versies, want een
/// besluit over een ouder moment landt op een oudere versie van de algemene wet.
fn stage_hook_outputs(service: &LawExecutionService) -> BTreeSet<String> {
    let mut outputs = BTreeSet::new();
    for law in service.resolver().all_law_versions() {
        for article in &law.articles {
            let fires = article.get_hooks().is_some_and(|hooks| {
                hooks.iter().any(|hook| {
                    hook.applies_to.legal_character.as_deref() == Some(BESCHIKKING)
                        && hook.applies_to.stage.as_deref() == Some(besluit::STAGE_BEKENDMAKING)
                })
            });
            if !fires {
                continue;
            }
            let Some(execution) = article.get_execution_spec() else {
                continue;
            };
            for output in execution.output.iter().flatten() {
                outputs.insert(output.name.clone());
            }
        }
    }
    outputs
}

/// Toets de `stage_uitkomsten` van het artikel dat een besluit uitvoert tegen
/// wat dat besluit en de hooks al leveren.
///
/// Een uitkomst die het besluit-gram al draagt, wordt bij een latere stage niet
/// opnieuw uitgerekend en niet overschreven: het besluit is genomen, en een
/// bekendmaking die er een andere waarde naast zet, zegt twee dingen over
/// hetzelfde. Een uitkomst die een hook op die stage ook levert, zou in het
/// stage-gram twee herkomsten hebben; en een vaste veldnaam van de stroom
/// [`BESCHIKKINGEN`] zou een veld van het gram overschrijven.
fn check_stage_uitkomsten(
    cell: &str,
    definition: &BesluitDefinition,
    stage_uitkomsten: &BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    hook_outputs: &BTreeSet<String>,
) -> Result<()> {
    let Some(eigen) = stage_uitkomsten
        .get(&definition.regulation)
        .and_then(|per_output| per_output.get(&definition.output))
    else {
        return Ok(());
    };
    let recorded = definition.recorded_outputs();
    let bezet = beschikkingen_fields();
    for output in eigen {
        let reason = if recorded.contains(output.as_str()) {
            "staat al in het gram van het besluit; een latere stage rekent haar niet opnieuw uit"
        } else if hook_outputs.contains(output) {
            "wordt ook door een hook op die stage geleverd, en zou dan twee herkomsten hebben"
        } else if bezet.contains(output.as_str()) {
            "heet als een vast veld van de stroom 'beschikkingen'"
        } else {
            continue;
        };
        return Err(SimulatorError::StageUitkomst {
            cell: cell.to_string(),
            besluit: definition.name.clone(),
            output: output.clone(),
            reason: reason.to_string(),
        });
    }
    Ok(())
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

/// De regelingen die één uitvoering werkelijk uitvoerde, met de versie die op
/// het moment van het besluit gold.
///
/// Drie dingen die dit *niet* is. Het is niet de regeling van het besluit alleen:
/// die levert zelden in haar eentje een uitkomst, en wie alleen haar noemt laat
/// de uitvoeringsregeling waar het bedrag vandaan komt buiten het verhaal. Het
/// is niet `scope.loaded_regulations` uit het receipt: daar staat elke versie in
/// die de cel geladen heeft, ook een versie die op dit moment niet gold en een
/// regeling die deze uitvoering niet geraakt heeft. En het is geen trace: wat
/// een aangeroepen regeling op háár beurt aanriep, staat er niet in — elke
/// uitvoering heeft haar eigen herkomst, en die uitvouwen zou van dit verhaal
/// een uitdraai maken. Dezelfde grens als bij [`Cell::explain_inputs`].
///
/// De versie van de regeling van het besluit komt uit de uitkomst zelf
/// ([`ArticleResult::regulation_valid_from`]): dat ís de versie waaronder er
/// zojuist gerekend is, en het gram legt haar onder `regulation_valid_from` al
/// zo vast — een tweede opzoeking ernaast zou het gram en het journaal over
/// dezelfde uitvoering iets anders kunnen laten zeggen. Voor een aangeroepen
/// regeling draagt de uitkomst geen versie, en dan is de resolver de enige bron:
/// bevraagd op hetzelfde moment als waarmee de engine haar koos (RFC-019 §3).
///
/// **Grens.** Een aanroep die de engine bewust oversloeg — een verplichte
/// parameter noemde niemand, dus de regeling is niet uitgevoerd en de input
/// werd een afwezigheid (RFC-036) — draagt dezelfde herkomst als een aanroep die
/// wél draaide, en staat er dus ook in. Dat onderscheid zit niet in
/// [`InputProvenance`]; zolang dat zo is, leest deze lijst als "de regelingen
/// die deze uitvoering aanriep".
fn executed_regulations(
    regulation: &str,
    regulation_valid_from: Option<&str>,
    result: &ArticleResult,
    resolver: &RuleResolver,
    op_moment: NaiveDate,
) -> Vec<ExecutedRegulation> {
    // De regeling van het besluit vooraan: díe uitvoering *is* het besluit. Wat
    // eronder hangt volgt op naam, zodat twee runs dezelfde volgorde geven.
    let called = result
        .input_provenance
        .values()
        .filter_map(|provenance| match provenance {
            InputProvenance::Regulation { regulation, .. } => Some(regulation.as_str()),
            InputProvenance::Parameter
            | InputProvenance::DataSource { .. }
            | InputProvenance::Cell { .. } => None,
        })
        .collect::<BTreeSet<&str>>();

    std::iter::once(ExecutedRegulation {
        regulation: regulation.to_string(),
        valid_from: regulation_valid_from.map(str::to_string),
    })
    .chain(
        called
            .into_iter()
            .filter(|called| *called != regulation)
            .map(|called| ExecutedRegulation {
                regulation: called.to_string(),
                valid_from: resolver
                    .get_law_for_date(called, Some(op_moment))
                    .and_then(|law| law.valid_from.clone()),
            }),
    )
    .collect()
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

    /// Geen geaccepteerde waarden: deze besluiten vragen niemand iets. Buiten
    /// een wereld om is er ook niemand die een vraag over de grens kan zetten —
    /// een cel kan dat zelf niet, en dat is het punt.
    fn no_accepted() -> BTreeMap<String, DecretogramInput> {
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

    /// De instellingen van de wereld waarin de besluiten hieronder vallen.
    ///
    /// `wet_op_de_zorgtoeslag` legt in haar lexogram een betalingsverplichting
    /// op met `ritme: $betalingsritme`, dus zonder deze instelling kan er op die
    /// wet geen besluit genomen worden. Voor de besluiten op een testregeling
    /// zonder verplichting doet ze niets.
    fn world_settings() -> BTreeMap<String, Value> {
        BTreeMap::from([(
            "betalingsritme".to_string(),
            Value::String("kwartaal".to_string()),
        )])
    }

    /// De cel die de verplichtingen van Dienst Toeslagen nakomt.
    ///
    /// Of die cel bestaat en of ze een betalingsstroom houdt, is een vraag van de
    /// wereld; een cel legt alleen vast wie er moet nakomen.
    fn world_payers() -> PartyBindings {
        let mut bindings = PartyBindings::default();
        bindings.bind("Dienst Toeslagen", "belastingdienst");
        bindings
    }

    /// Een besluit-definitie die alleen dient om in een melding genoemd te
    /// worden: de partijen van een verplichting lezen er niets anders uit dan
    /// haar naam en haar zaakkenmerk.
    fn payer_definition() -> BesluitDefinition {
        serde_yaml_ng::from_str(
            r"
name: toekenning
regulation: test_zonder_bevoegd_gezag
output: komt_in_aanmerking
zaakkenmerk: 'toekenning/{bsn}'
params:
  - name: bsn
    type: string
",
        )
        .unwrap_or_else(|e| panic!("testdefinitie moet parsen: {e}"))
    }

    /// Eén verplichting uit een lexogram dat geen bevoegd gezag aanwijst.
    fn declared_without_authority() -> DeclaredObligations {
        DeclaredObligations {
            origin: ObligationOrigin {
                regulation: "test_zonder_bevoegd_gezag".to_string(),
                valid_from: Some("2024-01-01".to_string()),
                article: "1".to_string(),
            },
            authority: None,
            article_outputs: BTreeSet::from(["komt_in_aanmerking".to_string()]),
            stage_outputs: BTreeSet::new(),
            items: vec![serde_yaml_ng::from_str(
                "soort: betaling\nbedrag: $komt_in_aanmerking\nritme: ineens\ngrondslag: art. 1\n",
            )
            .unwrap_or_else(|e| panic!("testverplichting moet parsen: {e}"))],
            vervanging: None,
        }
    }

    /// De schuldenaar is standaard het bevoegd gezag, dus een regeling die
    /// daarover zwijgt laat de verplichting zonder partij. Dat is iets anders dan
    /// een besluit zónder verplichting: daar valt niets na te komen, hier is er
    /// wel iets maar niemand om het aan te hangen.
    #[test]
    fn een_verplichting_zonder_bevoegd_gezag_wordt_geweigerd() {
        let err = declared_without_authority()
            .static_parties("toeslagen", &payer_definition())
            .expect_err("een verplichting zonder gezag hoort te falen");
        assert!(
            matches!(err, SimulatorError::ObligationWithoutAuthority { .. }),
            "verwachtte ObligationWithoutAuthority, kreeg {err}"
        );
    }

    /// Een partij die deze wereld niet als cel kent, is **geen** fout: de
    /// verplichting bestaat, er is alleen niemand die haar nakomt. Een
    /// terugvordering op een burger is precies dat geval.
    #[test]
    fn een_partij_zonder_cel_levert_geen_betaler_en_geen_fout() {
        assert_eq!(world_payers().cell_for("Minister van Financiën"), None);
    }

    /// En dat geldt voor de **schuldeiser** net zo goed. Zij komt alleen na als
    /// de richting omkeert, dus ze hoeft geen betalingsstroom te houden — maar
    /// een `#bevoegd_gezag` dat nergens op uitkomt is ook aan die kant een gat in
    /// de regeling, en dat hoort bij het optuigen te blijken en niet pas bij het
    /// eerste besluit.
    #[test]
    fn ook_een_schuldeiser_zonder_bevoegd_gezag_wordt_geweigerd() {
        let mut declared = declared_without_authority();
        // Alleen de schuldeiser noemt het gezag nog: de schuldenaar wijst naar een
        // parameter, dus zonder deze toets zou er hier niets langskomen.
        declared.items[0].schuldenaar = Some("$bsn".to_string());
        declared.items[0].schuldeiser = Some(besluit::BEVOEGD_GEZAG_REFERENCE.to_string());

        let err = declared
            .static_parties("toeslagen", &payer_definition())
            .expect_err("een schuldeiser zonder gezag hoort te falen");
        assert!(
            matches!(err, SimulatorError::ObligationWithoutAuthority { .. }),
            "verwachtte ObligationWithoutAuthority, kreeg {err}"
        );
    }

    /// De binding gaat over genormaliseerde tekst: een hoofdletter of een spatie
    /// vooraan is een schrijfwijze en geen andere organisatie.
    #[test]
    fn de_binding_kijkt_niet_naar_hoofdletters_of_spaties() {
        assert_eq!(
            world_payers().cell_for("  dienst toeslagen "),
            Some("belastingdienst")
        );
    }

    /// Is de schuldenaar een partij die zelf als cel meedoet, dan betaalt die
    /// cel — daarvoor hoeft niemand een `komt_na` op te schrijven.
    #[test]
    fn een_cel_die_de_naam_zelf_draagt_komt_haar_na() {
        let mut bindings = PartyBindings::default();
        bindings.note_identity("999993653", "aanvrager");
        assert_eq!(bindings.cell_for("999993653"), Some("aanvrager"));
    }

    /// En `komt_na` gaat vóór: een bestuursorgaan dat zelf besluit, laat
    /// doorgaans een ander systeem betalen, en dat systeem zegt dat zo.
    #[test]
    fn komt_na_gaat_voor_de_identiteit_van_een_cel() {
        let mut bindings = PartyBindings::default();
        bindings.note_identity("Dienst Toeslagen", "toeslagen");
        bindings.bind("Dienst Toeslagen", "belastingdienst");
        assert_eq!(
            bindings.cell_for("Dienst Toeslagen"),
            Some("belastingdienst")
        );
    }

    /// Dragen twee cellen dezelfde naam, dan wijst de identiteit niemand aan:
    /// kiezen zou een betaling bij een willekeurige organisatie laten landen.
    #[test]
    fn twee_cellen_met_dezelfde_naam_wijzen_niemand_aan() {
        let mut bindings = PartyBindings::default();
        bindings.note_identity("Dienst Toeslagen", "toeslagen");
        bindings.note_identity("dienst toeslagen", "toeslagen_twee");
        assert_eq!(bindings.cell_for("Dienst Toeslagen"), None);
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
identity: Dienst Toeslagen
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

    /// Een cel die één testregeling laadt, met de meegegeven `accepts_from`.
    ///
    /// De regeling staat in `fixtures/regulation/` en niet in het corpus: ze
    /// bestaat om een eigenschap van de opstelling te tonen (een
    /// `source.regulation` die een cel aanwijst), en dat is geen recht.
    fn toets_cel(accepts_from: &str) -> Result<Cell> {
        let config = config(&format!(
            r"
id: toeslagen
laws:
  - test_partnerschapstoets
{accepts_from}
"
        ));
        Cell::from_config(&config, &regulation_root(), &no_fixtures())
    }

    #[test]
    fn een_cel_bron_die_de_wet_noemt_wordt_geaccepteerd() {
        toets_cel(
            "accepts_from:
  - cell: brp
    output: partnerschap
    lexostatus: partnerschap
    field: partnerschap_type",
        )
        .unwrap_or_else(|e| panic!("deze afspraak hoort bij wat de wet vraagt: {e}"));
    }

    /// Een cel bevraagt alleen de cellen die haar eigen wetten noemen (I3). Een
    /// afspraak daarbuiten zet een vraaggraf open waar het recht niet om vraagt.
    #[test]
    fn een_cel_bron_die_geen_wet_noemt_wordt_geweigerd() {
        let err = toets_cel(
            "accepts_from:
  - cell: kadaster
    output: eigendom
    lexostatus: eigendom
    field: is_eigenaar",
        )
        .expect_err("een bron die geen wet aanwijst hoort te falen");
        let SimulatorError::UnclaimedCellSource { asked, .. } = &err else {
            panic!("verwachtte UnclaimedCellSource, kreeg {err}");
        };
        assert!(
            asked.contains("brp.partnerschap"),
            "de melding hoort te zeggen waar de wetten wél om vragen, kreeg: {asked}"
        );
    }

    #[test]
    fn twee_afspraken_over_dezelfde_bron_worden_geweigerd() {
        let err = toets_cel(
            "accepts_from:
  - cell: brp
    output: partnerschap
    lexostatus: partnerschap
    field: partnerschap_type
  - cell: brp
    output: partnerschap
    lexostatus: laatste_huwelijk
    field: partnerschap_type",
        )
        .expect_err("twee afspraken over dezelfde bron horen te falen");
        assert!(
            matches!(err, SimulatorError::DuplicateCellSource { .. }),
            "verwachtte DuplicateCellSource, kreeg {err}"
        );
    }

    /// Een besluit-definitie over een uitkomst die geen beschikking is, wordt
    /// bij het optuigen geweigerd.
    ///
    /// `vermogen_onder_grens` is een toets (`legal_character: TOETS`), geen
    /// besluit. Een decretogram is een uitkomst met `BESCHIKKING` (RFC-022
    /// §1.2); een toets als besluit vastleggen zou een gram in de stroom met
    /// beschikkingen leggen dat geen beschikking is.
    #[test]
    fn een_besluit_over_een_toets_wordt_geweigerd() {
        let err = Cell::from_config(
            &besluitende_cel(
                "    regulation: wet_op_de_zorgtoeslag
    output: vermogen_onder_grens
    zaakkenmerk: 'vermogen/{bsn}'
    params:
      - name: bsn
        type: string
    inputs:
      bsn:
        param: bsn",
            ),
            &regulation_root(),
            &no_fixtures(),
        )
        .expect_err("een besluit over een toets hoort te falen");
        let SimulatorError::BesluitNotABeschikking { output, found, .. } = &err else {
            panic!("verwachtte BesluitNotABeschikking, kreeg {err}");
        };
        assert_eq!(output, "vermogen_onder_grens");
        assert_eq!(
            found, "TOETS",
            "de melding zegt wat de wet er wél van maakt"
        );
    }

    /// Het gezag op het artikel gaat vóór het gezag op het document (RFC-002).
    ///
    /// `test_gezag_op_artikel` noemt op het document 'Documentgezag' en op het
    /// artikel dat het besluit voortbrengt 'Artikelgezag'. Eén wet kan meer dan
    /// één gezag kennen, dus het artikel beslist; wie het documentgezag draagt,
    /// mag dít besluit niet nemen.
    #[test]
    fn het_gezag_op_het_artikel_gaat_voor_het_gezag_op_het_document() {
        let config = config(
            r"
id: uitvoerder
laws:
  - test_gezag_op_artikel
chronicles:
  - stream: inkomensleveringen
    key: bsn
    events:
      - name: inkomenslevering
        intake: levering
        recording_actor: uitvoerder
        op_moment: 2023-11-15
        fields:
          bsn: '999993653'
          is_verzekerde: true
besluit_definitions:
  - name: toekenning
    regulation: test_gezag_op_artikel
    output: komt_in_aanmerking
    zaakkenmerk: 'toekenning/{bsn}'
    params:
      - name: bsn
        type: string
    inputs:
      bsn:
        param: bsn
      is_verzekerde:
        from_chronicle: inkomensleveringen
        field: is_verzekerde
",
        );
        let mut cell = Cell::from_config(&config, &regulation_root(), &no_fixtures())
            .unwrap_or_else(|e| panic!("de cel moet op te tuigen zijn: {e}"));

        let err = cell
            .decide(
                "toekenning",
                &bsn(),
                DecisionContext {
                    identity: "Documentgezag",
                    op_moment: moment(),
                    settings: &world_settings(),
                    parties: &world_payers(),
                },
                &no_accepted(),
                None,
            )
            .expect_err("het documentgezag mag dit artikel niet uitvoeren");
        let SimulatorError::NotCompetentAuthority { authority, .. } = &err else {
            panic!("verwachtte NotCompetentAuthority, kreeg {err}");
        };
        assert_eq!(
            authority, "Artikelgezag",
            "de weigering noemt het gezag van het artikel"
        );

        let gram = cell
            .decide(
                "toekenning",
                &bsn(),
                DecisionContext {
                    identity: "Artikelgezag",
                    op_moment: moment(),
                    settings: &world_settings(),
                    parties: &world_payers(),
                },
                &no_accepted(),
                None,
            )
            .unwrap_or_else(|e| panic!("het artikelgezag mag wél besluiten: {e}"));
        assert_eq!(gram.competent_authority.as_deref(), Some("Artikelgezag"));
        assert_eq!(gram.besloten_door, "Artikelgezag");
    }

    /// Een `#`-verwijzing die nergens op uitkomt, zet de toets niet stil.
    ///
    /// `test_gezag_onoplosbaar` wijst op het artikel naar `#bevoegd_gezag`, maar
    /// geen actie zet die uitkomst; het document noemt een ander gezag. Zou het
    /// platform dan naar het document terugvallen of het gezag als "niets" lezen,
    /// dan besloot hier een cel die de wet niet aanwijst — of iedereen.
    #[test]
    fn een_onoplosbare_verwijzing_naar_het_gezag_is_een_fout() {
        let config = config(
            r"
id: uitvoerder
laws:
  - test_gezag_onoplosbaar
chronicles:
  - stream: inkomensleveringen
    key: bsn
    events:
      - name: inkomenslevering
        intake: levering
        recording_actor: uitvoerder
        op_moment: 2023-11-15
        fields:
          bsn: '999993653'
          is_verzekerde: true
besluit_definitions:
  - name: toekenning
    regulation: test_gezag_onoplosbaar
    output: komt_in_aanmerking
    zaakkenmerk: 'toekenning/{bsn}'
    params:
      - name: bsn
        type: string
    inputs:
      bsn:
        param: bsn
      is_verzekerde:
        from_chronicle: inkomensleveringen
        field: is_verzekerde
",
        );
        let mut cell = Cell::from_config(&config, &regulation_root(), &no_fixtures())
            .unwrap_or_else(|e| panic!("de cel moet op te tuigen zijn: {e}"));

        for identity in ["Documentgezag", "uitvoerder"] {
            let err = cell
                .decide(
                    "toekenning",
                    &bsn(),
                    DecisionContext {
                        identity,
                        op_moment: moment(),
                        settings: &world_settings(),
                        parties: &world_payers(),
                    },
                    &no_accepted(),
                    None,
                )
                .expect_err("een onoplosbaar gezag hoort het besluit te laten omvallen");
            let SimulatorError::CompetentAuthorityUnresolvable { reference, .. } = &err else {
                panic!("verwachtte CompetentAuthorityUnresolvable, kreeg {err}");
            };
            assert_eq!(reference, "bevoegd_gezag");
        }
        assert_eq!(
            cell.chronicles.len_of(BESCHIKKINGEN),
            Some(0),
            "en er wordt niets vastgelegd"
        );
    }

    /// Het gram zegt op welke stand van welke eigen kroniek de uitvoering leunde.
    ///
    /// RFC-022 §1.3: een kroniek die aan een uitvoering bijdraagt, wordt met
    /// inhoud en versie vastgelegd. Zonder dat is een besluit niet te
    /// reproduceren — de waarden die de engine uit een kroniek las, staan
    /// nergens anders in het gram dan in de trace.
    #[test]
    fn het_decretogram_noemt_de_stand_van_elke_eigen_kroniek() {
        let mut cell = besluitende_toeslagen(VASTSTELLING);
        let gram = cell
            .decide(
                "zorgtoeslag_vaststelling",
                &bsn(),
                DecisionContext {
                    identity: IDENTITEIT,
                    op_moment: moment(),
                    settings: &world_settings(),
                    parties: &world_payers(),
                },
                &no_accepted(),
                None,
            )
            .unwrap_or_else(|e| panic!("het besluit moet genomen kunnen worden: {e}"));

        let bron = gram
            .chronicle_sources
            .iter()
            .find(|source| source.chronicle == "inkomensleveringen")
            .unwrap_or_else(|| panic!("de kroniek die als databron klaarstond hoort in het gram"));
        assert_eq!(
            bron.op_moment,
            moment(),
            "de stand is die van het moment van het besluit"
        );
        assert_eq!(bron.grams, 1);
        assert!(
            bron.content_hash.starts_with("sha256:"),
            "{}",
            bron.content_hash
        );
        assert!(
            gram.chronicle_sources
                .iter()
                .all(|source| source.chronicle != BESCHIKKINGEN),
            "de stroom met besluiten stond niet klaar en hoort er dus niet bij"
        );

        // Terug te lezen uit de kroniek: het gram draagt de lijst als vast veld.
        let event = cell
            .chronicles
            .last_recording(BESCHIKKINGEN)
            .unwrap_or_else(|| panic!("het gram ligt in de kroniek"));
        let Some(Value::Array(sources)) = event.fields.get(besluit::CHRONICLE_SOURCES) else {
            panic!(
                "het gram hoort '{}' als lijst te dragen",
                besluit::CHRONICLE_SOURCES
            );
        };
        assert_eq!(sources.len(), gram.chronicle_sources.len());
    }

    /// Een cel-id dat net zo heet als een eigen regeling zou een vraag voor de
    /// andere organisatie door die regeling laten beantwoorden, zonder spoor. De
    /// engine weigert dat ook; hier valt het bij het optuigen.
    #[test]
    fn een_cel_bron_die_een_eigen_regeling_overschaduwt_wordt_geweigerd() {
        let err = toets_cel(
            "accepts_from:
  - cell: test_partnerschapstoets
    output: partnerschap
    lexostatus: partnerschap
    field: partnerschap_type",
        )
        .expect_err("een cel-id dat een eigen regeling overschaduwt hoort te falen");
        assert!(
            matches!(err, SimulatorError::CellShadowsRegulation { .. }),
            "verwachtte CellShadowsRegulation, kreeg {err}"
        );
    }

    /// Een bron-cel met betalingen in haar kroniek.
    ///
    /// `bedrag` van de tweede vastlegging wordt letterlijk ingeplakt, zodat de
    /// test over een som die niet op te tellen valt dezelfde stroom gebruikt als
    /// de test die wél optelt.
    fn betaalcel(tweede_bedrag: &str) -> CellConfig {
        config(&format!(
            r"
id: belastingdienst
laws: []
chronicles:
  - stream: betalingen
    key: zaakkenmerk
    events:
      - name: betaling
        intake: betaling
        recording_actor: belastingdienst
        op_moment: 2024-01-01
        fields:
          zaakkenmerk: zorgtoeslag/999993653
          bedrag: 1000
          volgnummer: 1
      - name: betaling
        intake: betaling
        recording_actor: belastingdienst
        op_moment: 2024-04-01
        fields:
          zaakkenmerk: zorgtoeslag/999993653
          bedrag: {tweede_bedrag}
          volgnummer: 2
lexostatus_definitions:
  - name: betaald_tot_nu_toe
    inputs:
      - name: zaakkenmerk
        type: string
    outputs:
      - bedrag
    reduction:
      chronicle: betalingen
      key: zaakkenmerk
      sum: bedrag
"
        ))
    }

    fn zaak(zaakkenmerk: &str) -> BTreeMap<String, Value> {
        BTreeMap::from([(
            "zaakkenmerk".to_string(),
            Value::String(zaakkenmerk.to_string()),
        )])
    }

    /// De som is een reductie over de tijdas, net als elk ander kroniekfilter: ze
    /// telt op wat op het gevraagde moment vastlag en niet wat er inmiddels bij
    /// is gekomen.
    #[test]
    fn de_som_telt_op_wat_op_dat_moment_vastlag() {
        let cell = Cell::from_config(&betaalcel("500"), &regulation_root(), &no_fixtures())
            .unwrap_or_else(|e| panic!("de cel moet op te tuigen zijn: {e}"));

        let vroeg = cell
            .reduce(
                "betaald_tot_nu_toe",
                &zaak("zorgtoeslag/999993653"),
                date("2024-02-01"),
            )
            .unwrap_or_else(|e| panic!("de som moet te maken zijn: {e}"));
        assert_eq!(values(&vroeg).get("bedrag"), Some(&Value::Int(1000)));

        let laat = cell
            .reduce(
                "betaald_tot_nu_toe",
                &zaak("zorgtoeslag/999993653"),
                date("2024-06-01"),
            )
            .unwrap_or_else(|e| panic!("de som moet te maken zijn: {e}"));
        assert_eq!(values(&laat).get("bedrag"), Some(&Value::Int(1500)));
    }

    /// Nul vastleggingen is niet nul euro. Een 0 zou "er is niets betaald" niet
    /// kunnen onderscheiden van "over deze zaak ligt hier niets".
    #[test]
    fn de_som_over_niets_is_geen_nul() {
        let cell = Cell::from_config(&betaalcel("500"), &regulation_root(), &no_fixtures())
            .unwrap_or_else(|e| panic!("de cel moet op te tuigen zijn: {e}"));
        let answer = cell
            .reduce(
                "betaald_tot_nu_toe",
                &zaak("zorgtoeslag/999993999"),
                date("2024-06-01"),
            )
            .unwrap_or_else(|e| panic!("de som moet te maken zijn: {e}"));
        assert!(
            answer.not_established().is_some(),
            "verwachtte 'niets vastgesteld', kreeg {:?}",
            answer.outcome
        );
    }

    /// Een vastlegging zonder getal in het veld is een fout en geen nul: een som
    /// die haar overslaat valt stil te laag uit.
    #[test]
    fn een_som_over_een_veld_zonder_getal_faalt() {
        let cell = Cell::from_config(&betaalcel("veel"), &regulation_root(), &no_fixtures())
            .unwrap_or_else(|e| panic!("de cel moet op te tuigen zijn: {e}"));
        let err = cell
            .reduce(
                "betaald_tot_nu_toe",
                &zaak("zorgtoeslag/999993653"),
                date("2024-06-01"),
            )
            .expect_err("een som over tekst hoort te falen");
        assert!(
            matches!(err, SimulatorError::SumOfNonNumber { .. }),
            "verwachtte SumOfNonNumber, kreeg {err}"
        );
    }

    /// Een som levert precies één waarde op, dus `outputs` hoort precies dat veld
    /// te noemen. Al het andere is een belofte die nooit nagekomen wordt.
    #[test]
    fn een_som_die_iets_anders_publiceert_dan_ze_optelt_wordt_geweigerd() {
        let mut config = betaalcel("500");
        config.lexostatus_definitions[0].outputs = vec!["volgnummer".to_string()];
        let err = Cell::from_config(&config, &regulation_root(), &no_fixtures())
            .expect_err("een som die iets anders publiceert hoort te falen");
        assert!(
            matches!(err, SimulatorError::SumOutputMismatch { .. }),
            "verwachtte SumOutputMismatch, kreeg {err}"
        );
    }

    /// Het antwoord staat onder de naam die de definitie publiceert, ook als
    /// `sum` die naam anders schrijft.
    ///
    /// Het optuigen vergelijkt de twee zonder op kapitalen te letten, net als bij
    /// elke andere gepubliceerde uitkomst. Zou de som onder de naam uit `sum`
    /// antwoorden, dan lag het getal er wel maar onder een naam die de definitie
    /// niet noemt — en dan vindt een consument niets.
    #[test]
    fn de_som_antwoordt_onder_de_gepubliceerde_naam() {
        let mut config = betaalcel("500");
        config.lexostatus_definitions[0].outputs = vec!["Bedrag".to_string()];
        let cell = Cell::from_config(&config, &regulation_root(), &no_fixtures())
            .unwrap_or_else(|e| panic!("de cel moet op te tuigen zijn: {e}"));

        let answer = cell
            .reduce(
                "betaald_tot_nu_toe",
                &zaak("zorgtoeslag/999993653"),
                date("2024-06-01"),
            )
            .unwrap_or_else(|e| panic!("de som moet te maken zijn: {e}"));
        assert_eq!(values(&answer).get("Bedrag"), Some(&Value::Int(1500)));
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
identity: Dienst Toeslagen
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

    /// De naam waaronder de besluitende cel zich uitgeeft: wat haar
    /// veiligheidscontext bij een besluit meegeeft, en wat de zorgtoeslagwet als
    /// bevoegd gezag aanwijst.
    const IDENTITEIT: &str = "Dienst Toeslagen";

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

    /// Een tweede besluit ernaast dat het eerste over dezelfde zaak terugleest.
    ///
    /// Het leest `is_verzekerde` terug: dat is een **input** van het eerste gram,
    /// en die ligt daar een laag dieper dan de uitkomsten. Wat er getoetst wordt
    /// is dus beide wegen tegelijk — de zaak wordt gevonden, en het veld ook.
    const HERZIENING: &str = "  - name: zorgtoeslag_herziening
    regulation: wet_op_de_zorgtoeslag
    output: heeft_recht_op_zorgtoeslag
    zaakkenmerk: 'zorgtoeslag/{bsn}'
    params:
      - name: bsn
        type: string
    inputs:
      bsn:
        param: bsn
      is_verzekerde:
        from_decretogram: zorgtoeslag_vaststelling
        field: is_verzekerde";

    /// Een cel met het besluit én de herziening die het terugleest.
    fn herziene_toeslagen() -> Cell {
        besluitende_toeslagen(&format!("{VASTSTELLING}\n{HERZIENING}"))
    }

    /// Neem één besluit over deze persoon op dit moment.
    fn beslis(cell: &mut Cell, besluit: &str, op_moment: NaiveDate) -> Result<Decretogram> {
        cell.decide(
            besluit,
            &bsn(),
            DecisionContext {
                identity: IDENTITEIT,
                op_moment,
                settings: &world_settings(),
                parties: &world_payers(),
            },
            &no_accepted(),
            None,
        )
    }

    /// Het eerste besluit ligt er; dan leest de herziening het terug, met de
    /// herkomst die zegt dát het teruggelezen is.
    #[test]
    fn een_besluit_leest_een_eerder_besluit_over_dezelfde_zaak_terug() {
        let mut cell = herziene_toeslagen();
        beslis(&mut cell, "zorgtoeslag_vaststelling", moment())
            .unwrap_or_else(|e| panic!("het eerste besluit moet genomen kunnen worden: {e}"));

        // Op dezelfde dag: "op of vóór" telt het gram van vandaag mee.
        let gram = beslis(&mut cell, "zorgtoeslag_herziening", moment())
            .unwrap_or_else(|e| panic!("de herziening moet genomen kunnen worden: {e}"));

        let teruggelezen = gram
            .inputs
            .get("is_verzekerde")
            .unwrap_or_else(|| panic!("de teruggelezen input hoort in het gram te staan"));
        assert_eq!(teruggelezen.value, Value::Bool(true));
        assert_eq!(
            teruggelezen.origin,
            InputOrigin::EarlierDecretogram {
                besluit: "zorgtoeslag_vaststelling".to_string(),
                zaakkenmerk: "zorgtoeslag/999993653".to_string(),
                moment: moment(),
            },
            "de herkomst noemt het besluit, de zaak en het moment van dat besluit"
        );
    }

    /// Zonder eerder besluit is er niets om op terug te slaan. Dat is een fout en
    /// geen "niets vastgesteld": er wordt niets vastgelegd.
    #[test]
    fn een_herziening_zonder_eerder_besluit_faalt_en_legt_niets_vast() {
        let mut cell = herziene_toeslagen();
        let err = beslis(&mut cell, "zorgtoeslag_herziening", moment())
            .expect_err("zonder eerder besluit valt er niets terug te lezen");
        let SimulatorError::BesluitInputMissing { reason, .. } = &err else {
            panic!("verwachtte BesluitInputMissing, kreeg {err}");
        };
        assert!(
            reason.contains("geen eerder besluit 'zorgtoeslag_vaststelling'")
                && reason.contains("zorgtoeslag/999993653")
                && reason.contains("2025-01-01"),
            "de melding moet het besluit, de zaak en het moment noemen, kreeg: {reason}"
        );
        assert_eq!(
            cell.chronicles.len_of(BESCHIKKINGEN),
            Some(0),
            "een besluit dat niet doorging, hoort geen gram achter te laten"
        );
    }

    /// Het besluit van een ándere zaak telt niet mee: de sleutel is het
    /// zaakkenmerk van het lopende besluit, uit het eigen sjabloon.
    #[test]
    fn een_besluit_over_een_andere_zaak_wordt_niet_teruggelezen() {
        let mut cell = herziene_toeslagen();
        beslis(&mut cell, "zorgtoeslag_vaststelling", moment())
            .unwrap_or_else(|e| panic!("het eerste besluit moet genomen kunnen worden: {e}"));

        let andere_zaak =
            BTreeMap::from([("bsn".to_string(), Value::String("999993654".to_string()))]);
        let err = cell
            .decide(
                "zorgtoeslag_herziening",
                &andere_zaak,
                DecisionContext {
                    identity: IDENTITEIT,
                    op_moment: moment(),
                    settings: &world_settings(),
                    parties: &world_payers(),
                },
                &no_accepted(),
                None,
            )
            .expect_err("het besluit over een andere zaak hoort niet gepakt te worden");
        let SimulatorError::BesluitInputMissing { reason, .. } = &err else {
            panic!("verwachtte BesluitInputMissing, kreeg {err}");
        };
        assert!(
            reason.contains("zorgtoeslag/999993654"),
            "de melding hoort over de zaak van dít besluit te gaan, kreeg: {reason}"
        );
    }

    /// Liggen er twee grammen van dat besluit over dezelfde zaak, dan wint het
    /// laatste op of vóór dit moment — en de herkomst noemt dát moment, niet dat
    /// van het eerste.
    #[test]
    fn van_twee_eerdere_besluiten_wordt_het_laatste_teruggelezen() {
        let mut cell = herziene_toeslagen();
        let later = date("2025-03-01");
        for op_moment in [moment(), later] {
            beslis(&mut cell, "zorgtoeslag_vaststelling", op_moment)
                .unwrap_or_else(|e| panic!("het besluit van {op_moment} moet kunnen: {e}"));
        }

        let gram = beslis(&mut cell, "zorgtoeslag_herziening", date("2025-06-01"))
            .unwrap_or_else(|e| panic!("de herziening moet genomen kunnen worden: {e}"));

        let teruggelezen = gram
            .inputs
            .get("is_verzekerde")
            .unwrap_or_else(|| panic!("de teruggelezen input hoort in het gram te staan"));
        assert_eq!(
            teruggelezen.origin,
            InputOrigin::EarlierDecretogram {
                besluit: "zorgtoeslag_vaststelling".to_string(),
                zaakkenmerk: "zorgtoeslag/999993653".to_string(),
                moment: later,
            },
            "de herkomst hoort het laatste eerdere besluit te noemen"
        );
    }

    /// Een besluit van ná dit moment bestaat voor deze vraag niet — dezelfde
    /// tijdas als bij elke andere reductie.
    #[test]
    fn een_later_besluit_telt_niet_mee_bij_het_teruglezen() {
        let mut cell = herziene_toeslagen();
        beslis(&mut cell, "zorgtoeslag_vaststelling", moment())
            .unwrap_or_else(|e| panic!("het eerste besluit moet genomen kunnen worden: {e}"));

        let err = beslis(
            &mut cell,
            "zorgtoeslag_herziening",
            moment().pred_opt().unwrap_or_else(|| moment()),
        )
        .expect_err("een besluit van morgen telt vandaag niet mee");
        assert!(
            matches!(err, SimulatorError::BesluitInputMissing { .. }),
            "verwachtte BesluitInputMissing, kreeg {err}"
        );
    }

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
            .decide(
                "zorgtoeslag_vaststelling",
                &bsn(),
                DecisionContext {
                    identity: IDENTITEIT,
                    op_moment: moment(),
                    settings: &world_settings(),
                    parties: &world_payers(),
                },
                &no_accepted(),
                None,
            )
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
            gram.legal_character, BESCHIKKING,
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
            .decide(
                "zorgtoeslag_vaststelling",
                &bsn(),
                DecisionContext {
                    identity: IDENTITEIT,
                    op_moment: moment(),
                    settings: &world_settings(),
                    parties: &world_payers(),
                },
                &no_accepted(),
                None,
            )
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
            .decide(
                "zorgtoeslag_vaststelling",
                &bsn(),
                DecisionContext {
                    identity: IDENTITEIT,
                    op_moment: date("2024-01-01"),
                    settings: &world_settings(),
                    parties: &world_payers(),
                },
                &no_accepted(),
                None,
            )
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
            cell.decide(
                "zorgtoeslag_vaststelling",
                &bsn(),
                DecisionContext {
                    identity: IDENTITEIT,
                    op_moment: moment(),
                    settings: &world_settings(),
                    parties: &world_payers(),
                },
                &no_accepted(),
                None,
            )
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
            .decide(
                "zorgtoeslag_terugvordering",
                &bsn(),
                DecisionContext {
                    identity: IDENTITEIT,
                    op_moment: moment(),
                    settings: &world_settings(),
                    parties: &world_payers(),
                },
                &no_accepted(),
                None,
            )
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
        cell.decide(
            "zorgtoeslag_vaststelling",
            &bsn(),
            DecisionContext {
                identity: IDENTITEIT,
                op_moment: moment(),
                settings: &world_settings(),
                parties: &world_payers(),
            },
            &no_accepted(),
            None,
        )
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
