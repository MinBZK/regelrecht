//! De wereld: de cellen, de logische klok en de triggers die onderweg afgaan.
//!
//! Tijd is een eigenschap van de wereld en niet van een vraag. Eén logische
//! klok ([`NaiveDate`], nooit de wandklok) staat op een moment; [`World::advance`]
//! zet haar vooruit en loopt daarbij de triggers af die tussen het oude en het
//! nieuwe moment vallen, in datumvolgorde. Een trigger **voegt toe** en wijzigt
//! nooit een bestaand gram, dus het beeld van een eerder moment verandert niet
//! doordat de wereld verder loopt.
//!
//! Waarom in de wereld en niet in de cel: een cel kent alleen de momenten die
//! haar aangereikt worden. Zij houdt geen klok, precies zoals ze geen sleutels
//! en geen bevoegdheid houdt (RFC-022 §2).
//!
//! Er zijn twee trigger-soorten. Een [`Fixture`] met een `at`-datum wordt bij
//! het passeren vastgelegd, en een **vervallende verplichting** laat de cel die
//! haar draagt betalen. De lus is generiek — een gesorteerde lijst van
//! `(datum, trigger)` — zodat een soort erbij een variant erbij is en geen
//! andere klok. Dat de lijst gesorteerd blijft is een invariant en geen
//! toestand: een besluit tijdens de run plant nieuwe vervaldata, en die horen op
//! datumpositie en niet achteraan (zie [`World::plan`]).
//!
//! De wereld is ook de enige die een cel kan laten **besluiten**
//! ([`World::decide`]). Dat is geen trigger op de klok maar een aansturing van
//! buiten. De wereld houdt zelf geen register van wat er besloten is; het
//! decretogram ligt in de kroniek van de cel die besloot.
//!
//! Die aansturing komt uit het wereldbestand: een [`ActionDefinition`] is wat een
//! **actor** op de tijdlijn kan doen — een feit vastleggen (en het eventueel
//! leveren aan een ander), of een besluit-pad starten. [`World::act`] voert er
//! één uit, op de stand van de klok, met de waarden uit haar formulier.
//! Daarmee is de aansturing casusdata: een andere casus is een ander
//! wereldbestand en geen Rust.
//!
//! Naar buiten geeft [`World::snapshot`] één beeld van alles wat er staat — de
//! klok, de instellingen, per cel haar kronieken met elk gram, de acties die nu
//! kunnen, wat er over een celgrens ging en de waarschuwingen. Dat beeld is het
//! contract voor een frontend; het kent geen casus, want elk label komt uit het
//! wereldbestand.
//!
//! Daar komt één ding bij dat alleen de wereld kan: **accepteren**. Een besluit
//! dat een waarde van een andere organisatie nodig heeft, zegt dat — en de wereld
//! haalt hem op langs de veiligheidscontext van de besluitende cel en het
//! transport, want zij kent de peers en de cel niet. Wat over de grens ging komt
//! als [`DecisionRecord::crossings`] mee terug, zodat het meetinstrument het kan
//! zien zonder dat een cel het kent.

use crate::accept::CellBridge;
use crate::cell::{
    check_documented_params, BesluitDefinition, Cell, CellConfig, ChronicleEvent, Decretogram,
    DocumentedParameter, Intake, Lexostatus, ObligationDue, BETALINGEN, ZAAKKENMERK,
};
use crate::error::{Result, SimulatorError, Subject};
use crate::security::{Identity, SignedAnswer};
use crate::snapshot::{ActionState, Snapshot, WorldView};
use chrono::NaiveDate;
use regelrecht_engine::{CellResolver, Value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// De logische klok van een wereld, zoals het wereldbestand haar opgeeft.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    /// Het moment waarop de wereld begint. Verplicht: een wereld zonder
    /// startmoment zou op de wandklok moeten terugvallen, en dan is een run
    /// morgen een andere run.
    pub start: NaiveDate,
}

/// Eén startstand-gebeurtenis: wat er op welk moment wordt vastgelegd.
///
/// Een startstand is data. Fixtures op of vóór het startmoment van de klok staan
/// bij het optuigen al in de kroniek — de grens is inclusief, want een reductie
/// op het startmoment ziet wat op dat moment gebeurde. Latere fixtures zijn
/// triggers die afgaan wanneer [`World::advance`] hun datum passeert.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    /// Het moment waarop deze vastlegging gebeurt.
    pub at: NaiveDate,
    /// Wat er vastgelegd wordt, en bij wie.
    pub record: Recording,
}

/// Een vastlegging in de kroniek van één cel: de executogram-vorm van
/// RFC-022 §1.3, plus bij wie het gram landt.
///
/// `recording_actor` staat er niet bij: dat is [`Self::cell`]. Een kroniek
/// houdt alleen de eigen vastleggingen van de cel, dus die twee kunnen niet
/// uiteenlopen en hoeven niet twee keer opgeschreven.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    /// De cel die vastlegt, en in wiens kroniek het gram landt.
    pub cell: String,
    /// De kroniekstroom binnen die cel.
    pub chronicle: String,
    /// Wat er gebeurde.
    pub name: String,
    /// Het kanaal waarlangs het feit de cel bereikte.
    pub intake: Intake,
    /// Op welke grondslag dit feit vastgelegd wordt; mag leeg.
    #[serde(default)]
    pub grondslag: String,
    /// De vastgelegde velden.
    pub fields: BTreeMap<String, Value>,
}

impl Recording {
    /// Het executogram zoals het in de kroniek belandt, op het moment dat de
    /// klok passeert.
    fn event(&self, op_moment: NaiveDate) -> ChronicleEvent {
        ChronicleEvent {
            name: self.name.clone(),
            intake: self.intake,
            recording_actor: self.cell.clone(),
            grondslag: self.grondslag.clone(),
            op_moment,
            fields: self.fields.clone(),
        }
    }
}

/// Wat een actor op de tijdlijn kan doen.
///
/// Dit is de aansturing van de wereld, en ze is **casusdata**. Een actie noemt
/// wie haar doet, hoe ze heet voor een lezer, wat ze uitwerkt (een feit
/// vastleggen of een besluit-pad starten) en wanneer ze überhaupt kan. Wat er
/// niet in staat is even belangrijk: geen Rust, geen casusnaam in een type, en
/// geen eigen tijdstip — een actie gebeurt op de stand van de klok, want dat is
/// wat een actor doet.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "ActionFields")]
pub struct ActionDefinition {
    /// Waarmee de actie aangeroepen wordt; uniek in de wereld.
    pub id: String,
    /// De cel die de actie doet.
    ///
    /// Een actor is een cel: een burger, een partij en een betaalsysteem zijn in
    /// deze opstelling organisaties met een eigen kroniek (RFC-022 §1.3 — wie
    /// vastlegt, is de celbeheerder). Een actor buiten de cellen om zou een feit
    /// achterlaten dat niemand in zijn eigen journaal heeft.
    pub actor: String,
    /// Wat een lezer van deze actie ziet. Casusdata; de frontend verzint niets.
    pub label: String,
    /// Vrije toelichting; verschijnt in het beeld van de wereld naast het label.
    pub doc: Option<String>,
    /// Wat de actie uitwerkt.
    pub effect: ActionEffect,
    /// Wanneer deze actie kan; altijd, als het er niet staat.
    pub available_when: Option<Availability>,
}

/// De twee dingen die een actie kan uitwerken.
///
/// Precies één van de twee, en dat is geen beperking maar de scheiding die deze
/// opstelling maakt: een feit vastleggen is iets wat een cel *overkomt*, een
/// besluit nemen is iets wat ze *doet*. Eén actie die beide zou doen, zou die
/// twee in één gram laten vallen.
#[derive(Debug, Clone)]
pub enum ActionEffect {
    /// De actor legt een executogram vast in een eigen kroniek.
    Records(RecordsAction),
    /// De actor start het besluit-pad van een cel.
    ///
    /// Het formulier is dan niet dat van de actie maar dat van het besluit: de
    /// gedocumenteerde parameters die de besluit-definitie al noemt. Een tweede
    /// lijst ernaast zou ervan gaan afwijken.
    Decides(DecidesAction),
}

/// Het YAML-oppervlak van een actie: alle velden van beide vormen, los.
///
/// Dezelfde keuze als bij [`crate::Reduction`] en [`crate::BesluitInput`]: de
/// vorm valt hieronder en niet in serde, zodat er in de foutmelding staat wat er
/// mis is in plaats van "data did not match any variant" — en zodat een typfout
/// in een veldnaam alsnog geweigerd wordt.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionFields {
    /// Zie [`ActionDefinition::id`].
    id: String,
    /// Zie [`ActionDefinition::actor`].
    actor: String,
    /// Zie [`ActionDefinition::label`].
    label: String,
    /// Zie [`ActionDefinition::doc`].
    #[serde(default)]
    doc: Option<String>,
    /// Zie [`ActionEffect::Records`].
    #[serde(default)]
    records: Option<RecordsAction>,
    /// Zie [`ActionEffect::Decides`].
    #[serde(default)]
    decides: Option<DecidesAction>,
    /// Zie [`ActionDefinition::available_when`].
    #[serde(default)]
    available_when: Option<Availability>,
}

impl TryFrom<ActionFields> for ActionDefinition {
    type Error = String;

    fn try_from(fields: ActionFields) -> std::result::Result<Self, Self::Error> {
        let effect = match (fields.records, fields.decides) {
            (Some(_), Some(_)) => {
                return Err(format!(
                    "actie '{}' noemt zowel `records` als `decides`; een actie legt een \
                     feit vast (`records`) óf start een besluit (`decides`)",
                    fields.id
                ))
            }
            (Some(records), None) => ActionEffect::Records(records),
            (None, Some(decides)) => ActionEffect::Decides(decides),
            (None, None) => {
                return Err(format!(
                    "actie '{}' noemt geen `records` en geen `decides`, en werkt dus niets \
                     uit",
                    fields.id
                ))
            }
        };
        Ok(Self {
            id: fields.id,
            actor: fields.actor,
            label: fields.label,
            doc: fields.doc,
            effect,
            available_when: fields.available_when,
        })
    }
}

/// Een actie die een feit vastlegt, en het eventueel ook levert.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordsAction {
    /// De cel waarin het gram landt. Doorgaans de actor zelf.
    pub cell: String,
    /// De kroniekstroom binnen die cel.
    pub chronicle: String,
    /// Wat er gebeurde, in de woorden van de casus.
    pub name: String,
    /// Het kanaal waarlangs het feit de cel bereikte.
    pub intake: Intake,
    /// Op welke grondslag dit feit vastgelegd wordt; mag leeg.
    #[serde(default)]
    pub grondslag: String,
    /// Het formulier: wat de actor invult, en van welk type.
    ///
    /// Dezelfde vorm als de gedocumenteerde parameters van een lexostatus of een
    /// besluit, en om dezelfde reden: wie een waarde aanlevert die er niet in
    /// staat, of van het verkeerde type, wordt geweigerd.
    #[serde(default)]
    pub fields: Vec<DocumentedParameter>,
    /// Bij wie hetzelfde feit óók landt, als levering.
    #[serde(default)]
    pub delivers_to: Option<Delivery>,
}

/// De ontvangende kant van een feit dat een actor vastlegt.
///
/// Dit is de eerlijke vorm van een aanvraag: de aanvrager weet wat zij indiende,
/// de ontvanger weet wat hem geleverd is. Twee grammen, elk in de eigen kroniek
/// van de cel die het overkwam — en niet één gram dat twee cellen delen.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    /// De ontvangende cel.
    pub cell: String,
    /// De kroniekstroom waarin de levering landt.
    pub chronicle: String,
    /// Het kanaal waarlangs het feit bij de ontvanger binnenkomt.
    ///
    /// Standaard een levering; een aanvraag die bij het bestuursorgaan binnenkomt
    /// mag `aanvraag` zeggen, en dan staat dat in de kroniek van de ontvanger.
    #[serde(default = "levering")]
    pub intake: Intake,
    /// Hoe het gram bij de ontvanger heet; standaard dezelfde naam als bij de
    /// actor.
    #[serde(default)]
    pub name: Option<String>,
}

/// Het standaardkanaal van een levering.
fn levering() -> Intake {
    Intake::Levering
}

/// Een actie die het besluit-pad van een cel start.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecidesAction {
    /// De cel die besluit.
    pub cell: String,
    /// De besluit-definitie die uitgevoerd wordt.
    pub besluit: String,
}

/// Wanneer een actie kan: één simpele voorwaarde over een kroniek.
///
/// Leest als: *er ligt in kroniek `chronicle` van cel `cell` een feit waarin
/// veld `field` de waarde `equals` heeft.* Daarmee kan een actie wachten tot het
/// verhaal zover is — beslissen kan pas als er een aanvraag ligt — zonder dat de
/// volgorde in Rust vastgelegd wordt.
///
/// Met opzet klein. Dit is geen tweede reductietaal: er komt geen waarde naar
/// buiten, alleen ja of nee, en de vraag gaat over de wereld en niet over een
/// cel die een andere cel bevraagt.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Availability {
    /// De cel wiens kroniek nagekeken wordt.
    pub cell: String,
    /// De kroniekstroom.
    pub chronicle: String,
    /// Het veld waarop gekeken wordt.
    pub field: String,
    /// De waarde die dat veld moet hebben.
    pub equals: Value,
}

impl Availability {
    /// Leesbare voorwaarde, voor het beeld van de wereld en voor een weigering.
    fn describe(&self) -> String {
        format!(
            "in kroniek '{}' van cel '{}' ligt nog geen feit met {} = {}",
            self.chronicle, self.cell, self.field, self.equals
        )
    }
}

/// Een termijn die waarschuwt als een feit op tijd ontbreekt.
///
/// Nooit blokkerend, en dat is een standpunt en geen gemak: de wet zegt wat de
/// termijn was, niet dat het bestuursorgaan daarna niets meer mag. Een aanvraag
/// na de deadline is dus geen fout maar een waarschuwing naast de wereld, en het
/// besluit kan alsnog genomen worden.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deadline {
    /// Wat een lezer van deze termijn ziet. Casusdata.
    pub label: String,
    /// De dag waarop de termijn verstrijkt.
    ///
    /// Een datum en geen sjabloon over `settings`: de termijnen worden bij het
    /// optuigen op hun plek in de wachtrij gezet, en een instelling die daarna
    /// wijzigt zou een termijn moeten verplaatsen die misschien al gepasseerd is.
    pub at: NaiveDate,
    /// Het feit dat er dan hoort te liggen.
    pub warn_if_missing: ExpectedFact,
}

/// Het feit waarvan een termijn het ontbreken meldt.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedFact {
    /// De cel wiens kroniek nagekeken wordt.
    pub cell: String,
    /// De kroniekstroom.
    pub chronicle: String,
    /// De naam van het gram dat er hoort te liggen.
    pub name: String,
}

/// Eén gemiste termijn, zoals de wereld haar meldt.
#[derive(Debug, Clone, Serialize)]
pub struct Warning {
    /// Het label van de termijn uit het wereldbestand.
    pub label: String,
    /// De dag waarop de termijn verstreek.
    pub at: NaiveDate,
    /// De cel waar het feit hoorde te liggen.
    pub cell: String,
    /// De kroniekstroom waarin het hoorde te liggen.
    pub chronicle: String,
    /// De naam van het gram dat ontbrak.
    pub name: String,
}

impl Warning {
    /// Leesbare waarschuwing, voor een verslag.
    pub fn describe(&self) -> String {
        format!(
            "{} ({}): cel '{}' had op dat moment geen '{}' in kroniek '{}'",
            self.label, self.at, self.cell, self.name, self.chronicle
        )
    }
}

/// Eén feit dat tijdens een stap vastgelegd is.
#[derive(Debug, Clone)]
pub struct RecordedFact {
    /// De cel waarin het gram landde.
    pub cell: String,
    /// De kroniekstroom.
    pub chronicle: String,
    /// Het gram zelf.
    pub event: ChronicleEvent,
}

/// Wat er gebeurde tijdens één stap in de wereld.
///
/// Een stap is [`World::act`] of [`World::advance`], en wat er uit komt is wat een
/// lezer — of een latere web-laag — wil weten: welke grammen erbij kwamen, welke
/// besluiten genomen zijn en welke termijnen verstreken. Het is geen tweede
/// waarheid naast de kronieken: alles hierin ligt óók in de cel waar het hoort,
/// en [`World::snapshot`] is de stand.
#[derive(Debug, Clone, Default)]
pub struct Events {
    /// De feiten die vastgelegd zijn, in volgorde.
    pub recordings: Vec<RecordedFact>,
    /// De besluiten die genomen zijn, met wat er voor elk over een celgrens ging.
    pub decisions: Vec<DecisionRecord>,
    /// De termijnen die onderweg verstreken zonder dat het feit er lag.
    pub warnings: Vec<Warning>,
}

impl Events {
    /// Gebeurde er niets?
    pub fn is_empty(&self) -> bool {
        self.recordings.is_empty() && self.decisions.is_empty() && self.warnings.is_empty()
    }

    /// Leesbaar verslag van deze stap, één regel per gebeurtenis.
    pub fn describe(&self) -> String {
        let mut out = String::new();
        for fact in &self.recordings {
            let _ = writeln!(
                out,
                "        {}.{}: {} op {}",
                fact.cell, fact.chronicle, fact.event.name, fact.event.op_moment
            );
        }
        for decision in &self.decisions {
            let gram = &decision.decretogram;
            let _ = writeln!(
                out,
                "        {} besluit '{}' op {} -> zaakkenmerk '{}'",
                gram.cell, gram.besluit, gram.op_moment, gram.zaakkenmerk
            );
        }
        for warning in &self.warnings {
            let _ = writeln!(out, "        waarschuwing: {}", warning.describe());
        }
        out
    }
}

/// Wat er gebeurt als de klok een moment passeert.
///
/// De lus die ze afloopt is generiek, dus een soort erbij is een variant erbij
/// en geen andere klok.
#[derive(Debug, Clone)]
enum Trigger {
    /// Er wordt een executogram vastgelegd.
    Record(Recording),
    /// Een termijn van een verplichting vervalt en wordt nagekomen.
    ///
    /// Anders dan een [`Fixture`] staat deze niet in het wereldbestand: hij
    /// ontstaat tíjdens de run, op het moment dat een besluit de verplichting
    /// oplegt. Dat is meteen de reden dat [`World::pending`] gesorteerd moet
    /// blijven bij het bijzetten.
    Obligation(ObligationDue),
    /// Een termijn verstrijkt; ontbreekt het feit, dan komt er een waarschuwing.
    ///
    /// De plaats in het wereldbestand, want een waarschuwing hoort te vallen op
    /// het moment dat de termijn passeert en niet aan het eind van de run: een
    /// feit dat er een dag later wél ligt, lag er op de termijn niet.
    Deadline(usize),
}

/// Wat één besluit opleverde: het gram, en wat ervoor over de celgrens ging.
///
/// De twee horen bij elkaar en komen daarom samen naar buiten. Het gram draagt
/// de geaccepteerde waarden met hun herkomst — dat is wat de cel vastlegt — en
/// de contacten zijn het bewijs dat er precies die vragen gesteld zijn en geen
/// andere. Het observatielog is test-only en passief (zie
/// [`crate::observation`]), dus het kan dit niet zelf komen halen: wie besluit,
/// geeft het door, anders is het log stil incompleet in plaats van rood.
#[derive(Debug, Clone)]
pub struct DecisionRecord {
    /// Het vastgelegde besluit.
    pub decretogram: Decretogram,
    /// Elk contact over een celgrens dat dit besluit nodig had, in volgorde.
    /// Leeg als het besluit alles zelf wist.
    pub crossings: Vec<SignedAnswer>,
}

/// Het hele wereldbestand, los van de stappen die een scenario erop zet.
///
/// Dit is wat een wereld **is**: een klok, de cellen, de instellingen, de
/// startstand, de acties die een actor kan doen en de termijnen die kunnen
/// verstrijken. Eén type, want [`World::reset`] moet er opnieuw uit kunnen
/// opbouwen, en een web-laag moet er een verse wereld per sessie uit kunnen
/// maken. Zou de wereld alleen haar cellen bewaren, dan was "terug naar de
/// startstand" niet te doen zonder het bestand opnieuw te lezen.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldDefinition {
    /// De logische klok. Verplicht en expliciet: een run mag niet van de
    /// wandklok afhangen.
    pub clock: Clock,
    /// De cellen in deze wereld.
    pub cells: Vec<CellConfig>,
    /// De instellingen: casusdata die geen wet is.
    #[serde(default)]
    pub settings: BTreeMap<String, Value>,
    /// De startstand: vastleggingen met een moment.
    #[serde(default)]
    pub fixtures: Vec<Fixture>,
    /// Wat de actoren op de tijdlijn kunnen doen.
    #[serde(default)]
    pub actions: Vec<ActionDefinition>,
    /// De termijnen die waarschuwen als een feit ontbreekt.
    #[serde(default)]
    pub deadlines: Vec<Deadline>,
}

impl WorldDefinition {
    /// Lees een wereldbestand uit YAML.
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        Ok(serde_yaml_ng::from_str(yaml)?)
    }

    /// Lees een wereldbestand van schijf.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| SimulatorError::FileRead {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_yaml(&text)
    }
}

/// Eén gesimuleerde wereld: cellen, een klok, en wat er nog moet gebeuren.
///
/// De wereld bezit de cellen en is de enige die hun vastleg-pad kan aanroepen.
/// Naar buiten is [`World::reduce`] de weg naar een cel, met de klok als grens:
/// een moment ná de klok is een fout en geen voorspelling.
#[derive(Debug)]
pub struct World {
    /// Het wereldbestand waaruit deze wereld is opgetuigd.
    ///
    /// Bewaard en niet weggegooid, want [`World::reset`] bouwt er opnieuw uit op,
    /// en [`World::act`] leest er de actie uit die aangeroepen wordt. Dit is geen
    /// tweede waarheid naast de cellen: er staat niets in wat er tijdens de run
    /// gebeurd is.
    definition: WorldDefinition,
    /// Waar het corpus staat, zodat [`World::reset`] de cellen opnieuw kan
    /// opbouwen.
    regulation_root: PathBuf,
    /// De cellen, op id.
    cells: BTreeMap<String, Cell>,
    /// De instellingen van deze wereld: casusdata die geen wet is.
    ///
    /// Een betalingsritme is doorgaans beleid en geen wet, en beleid hoort niet
    /// in een besluit-definitie vast te staan alsof de wet het voorschrijft.
    /// Daarom staan ze hier, bij de wereld, en niet in een cel: een cel leest ze
    /// niet, ze krijgt ze aangereikt bij het besluit dat erop leunt.
    ///
    /// Apart van [`Self::definition`], want [`World::update_settings`] mag ze
    /// wijzigen en de startstand hoort daar niet mee te schuiven.
    settings: BTreeMap<String, Value>,
    /// De instellingen die al door een besluit gebruikt zijn, met dat besluit.
    ///
    /// Wat hierin staat, staat vast: zie [`World::update_settings`]. Het is geen
    /// schaduwboekhouding van de besluiten — het gram zelf ligt in de kroniek van
    /// de cel die besloot — maar de enige manier om te weten waar een instelling
    /// inmiddels onder ligt.
    used_settings: BTreeMap<String, (String, String)>,
    /// Waar de logische klok staat.
    clock: NaiveDate,
    /// Elk contact dat over een celgrens ging, in volgorde.
    ///
    /// Materiaal voor het meetinstrument, en niets meer: de cellen kunnen er niet
    /// bij, geen beslissing leunt erop, en wie hem weglaat verandert geen enkele
    /// uitkomst. Hij staat hier omdat een besluit ze aan de wereld teruggeeft en
    /// een beeld van de wereld ze hoort te kunnen tonen.
    crossings: Vec<SignedAnswer>,
    /// De termijnen die verstreken zonder dat het feit er lag.
    warnings: Vec<Warning>,
    /// Wat er nog moet gebeuren, oplopend op datum. Bij een gelijke datum
    /// beslist de volgorde waarin de triggers zijn opgegeven; de sortering is
    /// stabiel, dus dat is een vastgelegde eigenschap en geen toeval.
    ///
    /// Oplopend is een **invariant**, niet een toestand: [`World::fire_due`]
    /// leest alleen de kop. Wie hier later iets bij zet — een trigger die een
    /// volgende trigger inplant — moet dat op datumpositie doen en niet
    /// achteraan, anders gaat wat erbij komt te laat af of helemaal niet.
    pending: VecDeque<(NaiveDate, Trigger)>,
}

impl World {
    /// Tuig een wereld op: bouw de cellen, zet de klok op haar startmoment en
    /// leg vast wat vóór dat moment al gebeurd was.
    ///
    /// Fixtures met een datum tot en met het startmoment landen hier meteen in
    /// de kroniek: de klok staat op `start`, dus wat toen al gebeurd was, is
    /// gebeurd. De rest blijft staan tot [`World::advance`] eraan komt.
    ///
    /// Elke fixture wordt hier getoetst zoals ze bij het vastleggen getoetst zou
    /// worden — onbekende cel, onbekende stroom, ontbrekend sleutelveld — ook als
    /// haar datum nog jaren weg is. Een typfout in een startstand hoort niet
    /// halverwege een tijdlijn op te duiken, en of dat gebeurt mag niet afhangen
    /// van hoe ver die datum weg ligt.
    ///
    /// De verplichtingen van elk besluit worden hier ook getoetst, en om dezelfde
    /// reden: of een instelling bestaat en of de betalende cel een
    /// betalingsstroom houdt, is niet iets om op de eerste vervaldatum achter te
    /// komen. Zie [`check_obligations`].
    pub fn from_definition(definition: &WorldDefinition, regulation_root: &Path) -> Result<Self> {
        let configs = &definition.cells;
        let fixtures = &definition.fixtures;

        // Per cel, per stroom: de veldnamen die er van buiten de celconfiguratie
        // in komen — uit een `fixture` of uit het formulier van een actie. Een
        // stroom mag leeg opgetuigd worden en haar inhoud pas zo krijgen (zie
        // `scenarios/toeslagen_tijdlijn.yaml`); zonder dit zou
        // `Cell::from_config` zo'n stroom als veldloos zien en een
        // kroniekfilter daarop onterecht als typfout afkeuren.
        let mut fixture_fields: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> =
            BTreeMap::new();
        let mut add = |cell: &str, chronicle: &str, fields: Vec<String>| {
            fixture_fields
                .entry(cell.to_string())
                .or_default()
                .entry(chronicle.to_string())
                .or_default()
                .extend(fields);
        };
        for fixture in fixtures {
            let target = &fixture.record;
            add(
                &target.cell,
                &target.chronicle,
                target.fields.keys().cloned().collect(),
            );
        }
        for action in &definition.actions {
            let ActionEffect::Records(records) = &action.effect else {
                continue;
            };
            let names: Vec<String> = records
                .fields
                .iter()
                .map(|field| field.name.clone())
                .collect();
            add(&records.cell, &records.chronicle, names.clone());
            if let Some(delivery) = &records.delivers_to {
                add(&delivery.cell, &delivery.chronicle, names);
            }
        }

        let mut cells: BTreeMap<String, Cell> = BTreeMap::new();
        for config in configs {
            if cells.contains_key(&config.id) {
                return Err(SimulatorError::DuplicateCell {
                    cell: config.id.clone(),
                });
            }
            let fields = fixture_fields.get(&config.id).cloned().unwrap_or_default();
            cells.insert(
                config.id.clone(),
                Cell::from_config(config, regulation_root, &fields)?,
            );
        }

        for fixture in fixtures {
            let target = &fixture.record;
            let cell = cells
                .get(&target.cell)
                .ok_or_else(|| SimulatorError::UnknownCell {
                    cell: target.cell.clone(),
                })?;
            cell.check_recording(&target.chronicle, &target.event(fixture.at))?;
        }

        check_peers_exist(configs, &cells)?;
        check_obligations(configs, &cells, &definition.settings)?;
        check_actions(&definition.actions, &cells)?;
        check_deadlines(&definition.deadlines, &cells)?;

        let mut pending: Vec<(NaiveDate, Trigger)> = fixtures
            .iter()
            .map(|fixture| (fixture.at, Trigger::Record(fixture.record.clone())))
            .chain(
                definition
                    .deadlines
                    .iter()
                    .enumerate()
                    .map(|(index, deadline)| (deadline.at, Trigger::Deadline(index))),
            )
            .collect();
        pending.sort_by_key(|(at, _)| *at);

        let mut world = Self {
            definition: definition.clone(),
            regulation_root: regulation_root.to_path_buf(),
            cells,
            settings: definition.settings.clone(),
            used_settings: BTreeMap::new(),
            clock: definition.clock.start,
            crossings: Vec::new(),
            warnings: Vec::new(),
            pending: pending.into(),
        };
        world.fire_due(definition.clock.start)?;
        Ok(world)
    }

    /// Tuig de wereld opnieuw op: terug naar de startstand.
    ///
    /// Een verse wereld uit hetzelfde bestand, en niet een wereld die haar
    /// geschiedenis terugdraait. Dat verschil is de hele reden dat dit kan: een
    /// kroniek groeit en wijzigt nooit, dus "terug" bestaat niet — wat wél bestaat
    /// is opnieuw beginnen. Ook de instellingen gaan terug naar wat het bestand
    /// zegt; wie ze wijzigde, wijzigde de wereld en niet het bestand.
    pub fn reset(&mut self) -> Result<()> {
        *self = Self::from_definition(&self.definition.clone(), &self.regulation_root)?;
        Ok(())
    }

    /// Waar de logische klok staat.
    pub fn now(&self) -> NaiveDate {
        self.clock
    }

    /// De cellen van deze wereld, geleend en niet in bezit.
    ///
    /// `pub(crate)`, niet publiek: dit is geen tweede weg voor een consument om
    /// bij een cel te komen (dat blijft [`World::reduce`]), maar de ingang die
    /// `InProcessTransport` nodig heeft om een vraag over een celgrens te
    /// zetten. Het transport houdt de cellen zelf ook geleend — zie
    /// `transport.rs` — dus dit voegt geen tweede eigenaar toe.
    pub(crate) fn cells(&self) -> &BTreeMap<String, Cell> {
        &self.cells
    }

    /// Zet de klok vooruit naar `tot` en laat onderweg elke trigger afgaan.
    ///
    /// De triggers gaan af in datumvolgorde, en tijdens een trigger staat de
    /// klok op het moment van die trigger — een vastlegging krijgt dus haar
    /// eigen datum als `op_moment`, niet de eindstand. Achteruit loopt de klok
    /// niet: wie het beeld van een eerder moment wil, vraagt dat op met
    /// `op_moment` in [`World::reduce`].
    pub fn advance(&mut self, tot: NaiveDate) -> Result<Events> {
        if tot < self.clock {
            return Err(SimulatorError::ClockRunsBackwards {
                clock: self.clock.to_string(),
                to: tot.to_string(),
            });
        }
        let events = self.fire_due(tot)?;
        self.clock = tot;
        Ok(events)
    }

    /// Voer één actie uit, op de stand van de klok.
    ///
    /// Een actie heeft geen eigen moment: ze gebeurt nu. Dat is wat haar van een
    /// [`Fixture`] onderscheidt — een startstand staat op een datum, een actor
    /// doet iets op het moment dat de wereld staat — en het is ook wat maakt dat
    /// een actie geen gram in de toekomst kan leggen.
    ///
    /// `form_values` wordt tegen het formulier van de actie gehouden: precies de
    /// velden die ze documenteert, van het type dat ze noemt. Bij een actie die
    /// een besluit start, is dat formulier dat van het besluit zelf.
    ///
    /// Kan de actie nu niet ([`Availability`]), dan is dat een leesbare weigering
    /// en geen stilte: er staat in wat er nog niet vastligt.
    pub fn act(
        &mut self,
        action_id: &str,
        form_values: &BTreeMap<String, Value>,
    ) -> Result<Events> {
        let action = self
            .definition
            .actions
            .iter()
            .find(|candidate| candidate.id == action_id)
            .ok_or_else(|| SimulatorError::UnknownAction {
                action: action_id.to_string(),
                known: self
                    .definition
                    .actions
                    .iter()
                    .map(|candidate| candidate.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            })?
            .clone();

        if let Some(reason) = self.unavailable(&action) {
            return Err(SimulatorError::ActionNotAvailable {
                action: action.id.clone(),
                reason,
            });
        }

        check_documented_params(
            &action.actor,
            Subject::Actie,
            &action.id,
            &self.form(&action)?,
            form_values,
        )?;

        match &action.effect {
            ActionEffect::Records(records) => {
                let mut events = Events::default();
                for recording in recordings_of(&action, records, form_values) {
                    events
                        .recordings
                        .push(self.apply_recording(&recording, self.clock)?);
                }
                Ok(events)
            }
            ActionEffect::Decides(decides) => {
                let (record, mut events) = self.decide_and_settle(
                    &decides.cell,
                    &decides.besluit,
                    form_values,
                    self.clock,
                )?;
                events.decisions.push(record);
                Ok(events)
            }
        }
    }

    /// Wijzig de instellingen van deze wereld.
    ///
    /// Een instelling die al door een besluit gebruikt is, staat vast. Dat is geen
    /// voorzichtigheid maar wat een decretogram betekent: het gram legt vast
    /// waarop besloten is, dus een ritme dat er achteraf onder vandaan geschoven
    /// wordt, laat het gram iets anders zeggen dan er gebeurd is. Wie het toch wil
    /// wijzigen, begint een nieuwe wereld ([`World::reset`]).
    ///
    /// Alles of niets: is er één wijziging die niet kan, dan gaat er geen enkele
    /// door. Een half doorgevoerde wijziging zou een wereld achterlaten waarvan
    /// niemand kan zeggen welke instellingen er nu gelden.
    pub fn update_settings(&mut self, changes: &BTreeMap<String, Value>) -> Result<()> {
        let mut updated = self.settings.clone();
        for (setting, value) in changes {
            if !self.settings.contains_key(setting) {
                return Err(SimulatorError::UnknownWorldSetting {
                    setting: setting.clone(),
                    known: self
                        .settings
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
            if let Some((cell, besluit)) = self.used_settings.get(setting) {
                return Err(SimulatorError::SettingInUse {
                    setting: setting.clone(),
                    cell: cell.clone(),
                    besluit: besluit.clone(),
                });
            }
            updated.insert(setting.clone(), value.clone());
        }

        // Dezelfde toets als bij het optuigen: een ritme dat niet bestaat hoort
        // hier te vallen en niet bij het eerste besluit dat erop leunt.
        check_obligations(&self.definition.cells, &self.cells, &updated)?;
        self.settings = updated;
        Ok(())
    }

    /// Het beeld van de wereld: alles wat er nu staat, in één contract.
    ///
    /// Zie [`Snapshot`]. Het is een **inspectiebeeld**: de wereld toont wat waar
    /// ligt, geen cel komt eraan, en er verandert niets door het op te vragen.
    pub fn snapshot(&self) -> Snapshot {
        crate::snapshot::build(&WorldView {
            clock: self.clock,
            settings: &self.settings,
            used_settings: &self.used_settings,
            cells: &self.cells,
            actions: self.actions_now(),
            crossings: &self.crossings,
            warnings: &self.warnings,
        })
    }

    /// De waarschuwingen die tot nu toe zijn ontstaan, in volgorde.
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// Elke actie uit het wereldbestand, met haar formulier en of ze nu kan.
    ///
    /// Alle acties en niet alleen de mogelijke: een actie die nog niet kan, met de
    /// reden erbij, is wat een lezer nodig heeft om te zien waar het verhaal staat.
    /// Wie alleen de mogelijke toont, laat een keuze verdwijnen zonder te zeggen
    /// waarom.
    fn actions_now(&self) -> Vec<ActionState<'_>> {
        self.definition
            .actions
            .iter()
            .map(|action| ActionState {
                action,
                // Onbereikbaar leeg: het optuigen heeft elke actie aan haar cel en
                // haar besluit gebonden, dus het formulier is er.
                form: self.form(action).unwrap_or_default(),
                unavailable: self.unavailable(action),
            })
            .collect()
    }

    /// Het formulier van een actie: wat de actor invult, en van welk type.
    ///
    /// Bij een `records`-actie is dat wat de actie zelf noemt; bij een
    /// `decides`-actie zijn het de gedocumenteerde parameters van het besluit. Die
    /// tweede vorm heeft geen eigen lijst, en dat is met opzet: het besluit zegt al
    /// wat het nodig heeft, en twee lijsten zouden gaan afwijken.
    fn form(&self, action: &ActionDefinition) -> Result<Vec<DocumentedParameter>> {
        match &action.effect {
            ActionEffect::Records(records) => Ok(records.fields.clone()),
            ActionEffect::Decides(decides) => Ok(self
                .cell(&decides.cell)?
                .besluit_definition(&decides.besluit)?
                .params),
        }
    }

    /// Waarom deze actie nu niet kan; `None` als ze kan.
    fn unavailable(&self, action: &ActionDefinition) -> Option<String> {
        let condition = action.available_when.as_ref()?;
        // Onbereikbaar: het optuigen heeft de cel van de voorwaarde al gevonden.
        let cell = self.cells.get(&condition.cell)?;
        if cell.has_fact(
            &condition.chronicle,
            &condition.field,
            &condition.equals,
            self.clock,
        ) {
            return None;
        }
        Some(condition.describe())
    }

    /// Eén cel van deze wereld, of de fout die zegt dat ze er niet is.
    fn cell(&self, cell: &str) -> Result<&Cell> {
        self.cells
            .get(cell)
            .ok_or_else(|| SimulatorError::UnknownCell {
                cell: cell.to_string(),
            })
    }

    /// Vraag een gepubliceerde lexostatus aan één cel, op één moment.
    ///
    /// `op_moment` mag niet ná de klok liggen. Wat na de klok gebeurt heeft nog
    /// niets vastgelegd, dus een antwoord "op" zo'n moment zou een voorspelling
    /// zijn die zich voordoet als een reductie. Ervóór mag wel, en levert het
    /// beeld van toen: de cel laat feiten die pas later vastlagen buiten
    /// beschouwing.
    ///
    /// De wereld combineert niets. Ze zoekt de cel op en geeft het antwoord
    /// door zoals de cel het gaf; synthese over cellen heen hoort bij een
    /// consument (RFC-022 §4.1).
    ///
    /// Eerst de cel, dan het moment. Die volgorde is geen smaak: een vraag aan een
    /// cel die niet bestaat is een andere fout dan een vraag over een moment dat
    /// nog niet geweest is, en wie het moment vooropzet, noemt de onbekende cel in
    /// een melding over de klok — en stuurt de lezer naar de verkeerde regel in het
    /// bestand.
    pub fn reduce(
        &self,
        cell: &str,
        lexostatus: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<Lexostatus> {
        let found = self.cell(cell)?;
        if op_moment > self.clock {
            return Err(SimulatorError::MomentAfterClock {
                cell: cell.to_string(),
                subject: Subject::Lexostatus,
                name: lexostatus.to_string(),
                op_moment: op_moment.to_string(),
                clock: self.clock.to_string(),
            });
        }
        found.reduce(lexostatus, params, op_moment)
    }

    /// Laat één cel een besluit nemen, op één moment.
    ///
    /// De enige weg naar `Cell::decide`: net als vastleggen is besluiten iets
    /// dat een cel zelf doet, niet iets dat een consument bij haar bestelt. De
    /// wereld is hier de aansturing die er in de opstelling nog niet is — een
    /// actie op de tijdlijn, een verplichting die vervalt — en verder niets: ze
    /// kiest de cel op, geeft het moment door en houdt zelf geen register bij
    /// van wat er besloten is. Het decretogram ligt in de kroniek van de cel die
    /// besloot, en nergens anders.
    ///
    /// `op_moment` mag niet ná de klok liggen, om dezelfde reden als bij
    /// [`World::reduce`]: een besluit "op" een moment dat nog niet gebeurd is,
    /// zou een gram in de toekomst leggen.
    ///
    /// Dit is ook de enige plek waar een besluit de **celgrens over** kan. De
    /// cel zegt wat ze van een ander nodig heeft; de wereld — die de peers kent
    /// en de identiteit van de besluitende cel draagt — haalt het op langs de
    /// veiligheidscontext en het transport, en geeft het met herkomst terug. Wat
    /// erover de grens ging komt als [`DecisionRecord::crossings`] mee naar
    /// buiten: het observatielog staat buiten de band en kan het niet zelf komen
    /// halen.
    pub fn decide(
        &mut self,
        cell: &str,
        besluit: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<DecisionRecord> {
        self.decide_and_settle(cell, besluit, params, op_moment)
            .map(|(record, _)| record)
    }

    /// Hetzelfde besluit, met wat de verplichtingen eruit meteen opleverden.
    ///
    /// Apart van [`Self::decide`], omdat die twee verschillende vragen
    /// beantwoorden: een aanroeper die een besluit *neemt* wil het gram, en een
    /// aanroeper die een **actie** uitvoert wil weten wat er in die ene stap
    /// allemaal gebeurde — inclusief de eerste termijn, die op het moment van het
    /// besluit al vervalt.
    fn decide_and_settle(
        &mut self,
        cell: &str,
        besluit: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<(DecisionRecord, Events)> {
        // Eerst de cel, dan het moment: zie [`Self::reduce`].
        if !self.cells.contains_key(cell) {
            return Err(SimulatorError::UnknownCell {
                cell: cell.to_string(),
            });
        }
        if op_moment > self.clock {
            return Err(SimulatorError::MomentAfterClock {
                cell: cell.to_string(),
                subject: Subject::Besluit,
                name: besluit.to_string(),
                op_moment: op_moment.to_string(),
                clock: self.clock.to_string(),
            });
        }

        // De besluitende cel gaat uit de map, en de rest gaat naar de brug. Twee
        // vliegen: de brug kan de peers bezitten (een geregistreerde resolver
        // moet de uitvoering overleven, dus lenen kan niet), en de besluitende
        // cel is tijdens haar eigen besluit onbereikbaar voor het transport —
        // een cel die zichzelf over de grens bevraagt is hier geen afspraak maar
        // een lege plek.
        // Onbereikbaar leeg: het bestaan van de cel is hierboven al vastgesteld.
        let Some(mut deciding) = self.cells.remove(cell) else {
            return Err(SimulatorError::UnknownCell {
                cell: cell.to_string(),
            });
        };
        let bridge = Rc::new(CellBridge::new(
            Identity::for_cell(cell),
            besluit,
            deciding.accepts_from().cloned().collect::<Vec<_>>(),
            std::mem::take(&mut self.cells),
        ));

        let outcome = Self::accept_and_decide(
            &bridge,
            &mut deciding,
            besluit,
            params,
            &self.settings,
            op_moment,
        );

        // Ook als het besluit omviel: de wereld krijgt haar cellen terug zoals ze
        // waren. Een mislukt besluit legt niets vast, maar het mag al helemaal
        // geen cel laten verdwijnen.
        self.cells = bridge.release();
        self.cells.insert(cell.to_string(), deciding);

        // Bij een fout gaan de contacten mee weg. Dat kan zolang een omgevallen
        // besluit de run afbreekt — de scenario-runner geeft de fout door en stopt
        // — en de fout zelf al zegt bij wie het misging. Wordt hier ooit een
        // aanroeper op gezet die na een mislukt besluit doorgaat (een HTTP-laag
        // die er een foutantwoord van maakt), dan mist het log precies de vragen
        // van de besluiten die niet lukten, en dat is de gevaarlijke kant op: dan
        // horen de contacten met de fout mee naar buiten.
        let decretogram = outcome?;

        // De verplichtingen uit dit besluit worden triggers. Een termijn die nu
        // al vervalt — en de eerste termijn valt op het moment van het besluit,
        // tenzij `from` anders zegt — gaat meteen af: de klok staat er al, dus
        // wachten zou hem tot de volgende `advance` laten liggen en dan pas met
        // terugwerkende kracht laten vastleggen.
        //
        // Dit moet ná het herstel van `self.cells` hierboven: `settle` zoekt
        // zowel de betalende als de besluitende cel op via `self.cells`, en de
        // besluitende cel stond tot dat herstel nog niet terug, en de rest zat
        // nog in de brug.
        for due in &decretogram.obligations {
            self.plan(due.vervaldatum, Trigger::Obligation(due.clone()));
        }
        let events = self.fire_due(self.clock)?;

        // Welke instellingen dit besluit gebruikte, staan daarmee vast. Dat moet
        // hier, bij het besluit dat gelukt is: een besluit dat omviel heeft niets
        // vastgelegd en hoeft dus niets vast te zetten.
        for setting in self.settings_of(cell, besluit) {
            self.used_settings
                .insert(setting, (cell.to_string(), besluit.to_string()));
        }

        let crossings = bridge.crossings();
        self.crossings.extend(crossings.iter().cloned());

        Ok((
            DecisionRecord {
                decretogram,
                crossings,
            },
            events,
        ))
    }

    /// De instellingen waarop dit besluit leunde, uit het wereldbestand.
    ///
    /// Uit de definitie en niet uit het gram: het gram draagt het uitgerekende
    /// schema, en daaruit is niet meer te zien of het ritme uit een instelling
    /// kwam of letterlijk in de definitie stond. Een besluit dat de cel niet kent
    /// kwam hier nooit, dus een leeg antwoord is hier geen stilte.
    fn settings_of(&self, cell: &str, besluit: &str) -> Vec<String> {
        self.definition
            .cells
            .iter()
            .filter(|config| config.id == cell)
            .flat_map(|config| &config.besluit_definitions)
            .filter(|definition| definition.name == besluit)
            .flat_map(BesluitDefinition::settings_used)
            .map(str::to_string)
            .collect()
    }

    /// Haal op wat het besluit van anderen nodig heeft, en laat de cel besluiten.
    ///
    /// De volgorde is de hele bewijslast van invariant I5: eerst vragen, dan
    /// rekenen. Wat een ander vaststelt, komt binnen als waarde met herkomst en
    /// gaat als parameter de engine in; de logica van die ander draait hier
    /// niet. En wat de *wet* bij een andere cel haalt (tier 3), loopt langs
    /// dezelfde brug, die de besluit-engine voor de duur van dit besluit krijgt.
    fn accept_and_decide(
        bridge: &Rc<CellBridge>,
        deciding: &mut Cell,
        besluit: &str,
        params: &BTreeMap<String, Value>,
        settings: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<Decretogram> {
        let requests = deciding.acceptance_requests(besluit, params)?;
        let accepted = bridge.accept_all(&requests, op_moment)?;
        let shared: Rc<CellBridge> = Rc::clone(bridge);
        let resolver: Rc<dyn CellResolver> = shared;
        deciding.decide(
            besluit,
            params,
            settings,
            op_moment,
            &accepted,
            Some(resolver),
        )
    }

    /// Zet een trigger op zijn datumpositie in de wachtrij.
    ///
    /// [`Self::pending`] is oplopend, en dat is een invariant: [`Self::fire_due`]
    /// leest alleen de kop. Achteraan bijzetten zou een vervaldatum die vóór een
    /// al wachtende trigger valt te laat of helemaal niet laten afgaan. Bij een
    /// gelijke datum komt de nieuwe achter de bestaande — dezelfde regel als
    /// overal: wie later vastlegt, legt later vast.
    fn plan(&mut self, at: NaiveDate, trigger: Trigger) {
        let position = self.pending.partition_point(|(pending, _)| *pending <= at);
        self.pending.insert(position, (at, trigger));
    }

    /// Laat alles afgaan wat op of vóór `tot` valt, in datumvolgorde.
    ///
    /// De klok schuift mee naar het moment van de trigger die afgaat, zodat een
    /// trigger die zelf iets vastlegt dat op zijn eigen moment doet.
    ///
    /// Eén voor één van de kop af, en niet een hele kop in één keer weggehaald:
    /// valt een trigger om, dan blijven de triggers ná hem gewoon staan in
    /// plaats van met de fout te verdwijnen.
    fn fire_due(&mut self, tot: NaiveDate) -> Result<Events> {
        let mut events = Events::default();
        while let Some((at, trigger)) = self.next_due(tot) {
            self.clock = self.clock.max(at);
            match trigger {
                Trigger::Record(recording) => {
                    events
                        .recordings
                        .push(self.apply_recording(&recording, at)?);
                }
                Trigger::Obligation(due) => events.recordings.extend(self.settle(&due)?),
                Trigger::Deadline(index) => events.warnings.extend(self.check_deadline(index, at)),
            }
        }
        self.warnings.extend(events.warnings.iter().cloned());
        Ok(events)
    }

    /// Leg één vastlegging vast, in de cel en de stroom die ze noemt.
    ///
    /// Eén plek voor de weg naar de kroniek, want twee dingen komen hier uit: een
    /// [`Fixture`] die de klok passeert, en een [`ActionDefinition`] die een actor
    /// nu doet. Zouden die uit elkaar lopen, dan zou een actie een gram kunnen
    /// leggen dat een startstand niet mag leggen.
    fn apply_recording(&mut self, recording: &Recording, at: NaiveDate) -> Result<RecordedFact> {
        let event = recording.event(at);
        let cell =
            self.cells
                .get_mut(&recording.cell)
                .ok_or_else(|| SimulatorError::UnknownCell {
                    cell: recording.cell.clone(),
                })?;
        cell.record(&recording.chronicle, event.clone())?;
        Ok(RecordedFact {
            cell: recording.cell.clone(),
            chronicle: recording.chronicle.clone(),
            event,
        })
    }

    /// Verstrijkt hier een termijn zonder dat het feit er ligt?
    ///
    /// Levert een waarschuwing en nooit een fout: de wet zegt wat de termijn was,
    /// niet dat er daarna niets meer mag (zie [`Deadline`]). Ligt het feit er wel,
    /// dan gebeurt er niets — een gehaalde termijn is geen gebeurtenis.
    fn check_deadline(&self, index: usize, at: NaiveDate) -> Option<Warning> {
        // Onbereikbaar leeg: de trigger is bij het optuigen op zijn index gezet.
        let deadline = self.definition.deadlines.get(index)?;
        let expected = &deadline.warn_if_missing;
        // Onbereikbaar leeg: het optuigen heeft de cel al gevonden.
        let cell = self.cells.get(&expected.cell)?;
        if cell.has_recording_named(&expected.chronicle, &expected.name, at) {
            return None;
        }
        Some(Warning {
            label: deadline.label.clone(),
            at,
            cell: expected.cell.clone(),
            chronicle: expected.chronicle.clone(),
            name: expected.name.clone(),
        })
    }

    /// Laat een vervallen termijn nakomen, aan beide kanten.
    ///
    /// De betalende cel legt vast dat zij betaalde, de besluitende dat het haar
    /// gemeld is. Twee vastleggingen, elk in de eigen kroniek van de cel die ze
    /// deed: niemand kopieert andermans staat, en beide kanten weten wat er
    /// gebeurde.
    ///
    /// Lag de betaling er al, dan gebeurt er niets — ook niet aan de andere kant.
    /// Dat is de idempotentie per volgnummer: dezelfde zaak met hetzelfde
    /// volgnummer is dezelfde termijn, en die wordt één keer nagekomen. Is de
    /// betaler de besluitende cel zelf, dan blijft het bij de betaling: een
    /// melding aan jezelf over wat je zelf deed is geen tweede feit.
    fn settle(&mut self, due: &ObligationDue) -> Result<Vec<RecordedFact>> {
        let payer = self
            .cells
            .get_mut(&due.payer)
            .ok_or_else(|| SimulatorError::UnknownCell {
                cell: due.payer.clone(),
            })?;
        if !payer.pay_obligation(due)? {
            return Ok(Vec::new());
        }
        let mut facts = vec![RecordedFact {
            cell: due.payer.clone(),
            chronicle: BETALINGEN.to_string(),
            event: due.payment_event(),
        }];

        if self
            .cells
            .get_mut(&due.decided_by)
            .ok_or_else(|| SimulatorError::UnknownCell {
                cell: due.decided_by.clone(),
            })?
            .note_obligation_paid(due)?
        {
            facts.push(RecordedFact {
                cell: due.decided_by.clone(),
                chronicle: BETALINGEN.to_string(),
                event: due.delivery_event(),
            });
        }
        Ok(facts)
    }

    /// De volgende trigger die op of vóór `tot` valt, van de kop van de lijst.
    ///
    /// De kop is genoeg omdat [`Self::pending`] oplopend is; is de eerste nog
    /// niet vervallen, dan is geen van de volgende dat.
    fn next_due(&mut self, tot: NaiveDate) -> Option<(NaiveDate, Trigger)> {
        let at = self.pending.front().map(|(at, _)| *at)?;
        if at > tot {
            return None;
        }
        self.pending.pop_front()
    }

    /// Hoeveel triggers nog niet afgegaan zijn.
    ///
    /// Voor het verslag van een run: een vastlegging die op geen enkel moment
    /// gevraagd wordt, gebeurt nooit, en dat hoort te zien te zijn in plaats van
    /// stil te blijven.
    pub fn pending_triggers(&self) -> usize {
        self.pending.len()
    }
}

/// Bestaat elke cel waarvan er in deze wereld geaccepteerd wordt?
///
/// Twee soorten afspraak wijzen een peer aan: `accept_from` in een
/// besluit-definitie en `accepts_from` op de cel zelf (tier 3). Geen van beide
/// is bij het optuigen van de cel te controleren — een cel kent geen andere cel
/// — dus het valt hier, bij de enige die ze allemaal kent.
fn check_peers_exist(configs: &[CellConfig], cells: &BTreeMap<String, Cell>) -> Result<()> {
    let known = || {
        cells
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    };
    for config in configs {
        let peers = config
            .besluit_definitions
            .iter()
            .flat_map(|definition| {
                definition.inputs.iter().filter_map(move |(input, origin)| {
                    let cell = origin.accepts_from()?;
                    Some((
                        cell,
                        format!("input '{input}' van besluit '{}'", definition.name),
                    ))
                })
            })
            .chain(config.accepts_from.iter().map(|source| {
                (
                    source.cell.as_str(),
                    format!("uitkomst '{}' van haar wetten", source.output),
                )
            }));

        for (peer, what) in peers {
            if !cells.contains_key(peer) {
                return Err(SimulatorError::UnknownAcceptedCell {
                    cell: config.id.clone(),
                    peer: peer.to_string(),
                    what,
                    known: known(),
                });
            }
        }
    }
    Ok(())
}

/// Toets de verplichtingen van elk besluit tegen de wereld waarin ze staan.
///
/// Wat een cel zelf kan nakijken — is het bedrag een eigen uitkomst, bestaat dit
/// ritme, levert `from` een datum op — is bij het optuigen van de cel al
/// gebeurd. Wat er hier bij komt, weet alleen de wereld:
///
/// - verwijst `schedule: $naam` naar een instelling die bestaat, en is die een
///   ritme;
/// - bestaat de betalende cel;
/// - houden de betalende én de besluitende cel een stroom [`BETALINGEN`] met
///   [`ZAAKKENMERK`] als sleutel? Beide leggen op een vervaldatum vast, en een
///   stroom die er niet is zou dat op de eerste vervaldatum laten omvallen —
///   halverwege de tijdlijn, in plaats van hier.
fn check_obligations(
    configs: &[CellConfig],
    cells: &BTreeMap<String, Cell>,
    settings: &BTreeMap<String, Value>,
) -> Result<()> {
    for config in configs {
        for definition in &config.besluit_definitions {
            definition.check_settings(&config.id, settings)?;

            let payers = definition.payers();
            if payers.is_empty() {
                continue;
            }

            // De besluitende cel legt de melding vast dat er betaald is, dus zij
            // heeft de stroom net zo goed nodig als de betaler.
            for holder in payers
                .iter()
                .copied()
                .chain(std::iter::once(config.id.as_str()))
            {
                let found = match cells.get(holder) {
                    None => {
                        return Err(SimulatorError::UnknownCell {
                            cell: holder.to_string(),
                        })
                    }
                    Some(cell) => cell.stream_key(BETALINGEN),
                };
                let reason = match found {
                    Some(key) if key == ZAAKKENMERK => continue,
                    Some(key) => format!("die stroom heeft sleutel '{key}'"),
                    None => "die cel houdt geen stroom met die naam".to_string(),
                };
                return Err(SimulatorError::ObligationStream {
                    cell: config.id.clone(),
                    besluit: definition.name.clone(),
                    holder: holder.to_string(),
                    expected: format!(
                        "een kroniekstroom '{BETALINGEN}' met sleutel '{ZAAKKENMERK}'"
                    ),
                    found: reason,
                });
            }
        }
    }
    Ok(())
}

/// De vastleggingen die één `records`-actie voortbrengt.
///
/// Eén of twee: het gram bij de actor, en — als de actie levert — hetzelfde feit
/// bij de ontvanger. Twee grammen en niet één gedeeld gram, want een kroniek
/// draagt wat een cel zélf overkwam (RFC-022 §1.3): de aanvrager weet wat ze
/// indiende, de ontvanger weet wat hem geleverd is.
fn recordings_of(
    action: &ActionDefinition,
    records: &RecordsAction,
    values: &BTreeMap<String, Value>,
) -> Vec<Recording> {
    let own = Recording {
        cell: records.cell.clone(),
        chronicle: records.chronicle.clone(),
        name: records.name.clone(),
        intake: records.intake,
        grondslag: records.grondslag.clone(),
        fields: values.clone(),
    };
    let Some(delivery) = &records.delivers_to else {
        return vec![own];
    };
    let delivered = Recording {
        cell: delivery.cell.clone(),
        chronicle: delivery.chronicle.clone(),
        name: delivery
            .name
            .clone()
            .unwrap_or_else(|| records.name.clone()),
        intake: delivery.intake,
        // De grondslag van de ontvanger is niet die van de actor: hij legt vast
        // dat een ander hem iets leverde, en de actie waarlangs dat gebeurde is
        // precies wat daarvan te zeggen valt.
        grondslag: format!(
            "levering door cel '{}' met actie '{}'",
            action.actor, action.id
        ),
        fields: values.clone(),
    };
    vec![own, delivered]
}

/// Toets de acties tegen de wereld waarin ze staan.
///
/// Alles wat een actie belooft, blijkt hier en niet bij de eerste aanroep: bestaat
/// de actor, bestaat de cel, kan de stroom het gram dragen, bestaat het besluit,
/// en gaat de voorwaarde over een veld dat bestaat. Een wereldbestand met een
/// typfout in een actie hoort niet te laden.
fn check_actions(actions: &[ActionDefinition], cells: &BTreeMap<String, Cell>) -> Result<()> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for action in actions {
        if !seen.insert(action.id.as_str()) {
            return Err(SimulatorError::DuplicateAction {
                action: action.id.clone(),
            });
        }
        if !cells.contains_key(&action.actor) {
            return Err(SimulatorError::UnknownCell {
                cell: action.actor.clone(),
            });
        }

        match &action.effect {
            ActionEffect::Records(records) => {
                let fields: BTreeSet<String> = records
                    .fields
                    .iter()
                    .map(|field| field.name.clone())
                    .collect();
                check_recordable(
                    &action.id,
                    &records.cell,
                    &records.chronicle,
                    &fields,
                    cells,
                )?;
                if let Some(delivery) = &records.delivers_to {
                    if delivery.cell == records.cell {
                        return Err(SimulatorError::DeliveryToSelf {
                            action: action.id.clone(),
                            cell: delivery.cell.clone(),
                        });
                    }
                    check_recordable(
                        &action.id,
                        &delivery.cell,
                        &delivery.chronicle,
                        &fields,
                        cells,
                    )?;
                }
            }
            ActionEffect::Decides(decides) => {
                cells
                    .get(&decides.cell)
                    .ok_or_else(|| SimulatorError::UnknownCell {
                        cell: decides.cell.clone(),
                    })?
                    .besluit_definition(&decides.besluit)?;
            }
        }

        if let Some(condition) = &action.available_when {
            cells
                .get(&condition.cell)
                .ok_or_else(|| SimulatorError::UnknownCell {
                    cell: condition.cell.clone(),
                })?
                .check_stream_field(
                    Subject::Actie,
                    &action.id,
                    &condition.chronicle,
                    &condition.field,
                )?;
        }
    }
    Ok(())
}

/// Kan deze cel het gram van deze actie dragen?
///
/// De cel weet waarom niet; de wereld weet welke actie het was. Hier komen die
/// twee in één melding samen.
fn check_recordable(
    action: &str,
    cell: &str,
    chronicle: &str,
    fields: &BTreeSet<String>,
    cells: &BTreeMap<String, Cell>,
) -> Result<()> {
    cells
        .get(cell)
        .ok_or_else(|| SimulatorError::UnknownCell {
            cell: cell.to_string(),
        })?
        .check_recordable(chronicle, fields)
        .map_err(|reason| SimulatorError::ActionRecording {
            action: action.to_string(),
            cell: cell.to_string(),
            stream: chronicle.to_string(),
            reason,
        })
}

/// Toets de termijnen tegen de wereld waarin ze staan.
///
/// Een termijn die naar een cel of een stroom wijst die er niet is, waarschuwt
/// altijd — over een feit dat nergens kon liggen. Dat hoort bij het optuigen te
/// vallen, niet als een waarschuwing die niemand kan wegnemen.
fn check_deadlines(deadlines: &[Deadline], cells: &BTreeMap<String, Cell>) -> Result<()> {
    for deadline in deadlines {
        let expected = &deadline.warn_if_missing;
        let cell = cells
            .get(&expected.cell)
            .ok_or_else(|| SimulatorError::UnknownCell {
                cell: expected.cell.clone(),
            })?;
        cell.check_stream(Subject::Termijn, &deadline.label, &expected.chronicle)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::ObligationDefinition;
    use crate::corpus::regulation_root;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
    }

    /// Een wereldbestand zonder acties en zonder termijnen.
    ///
    /// Die twee komen er per test bij die erover gaat; de tests hieronder gaan
    /// over de klok, de triggers en de verplichtingen, en die hoeven niet elke
    /// keer een leeg `actions:` op te schrijven.
    fn definition(
        cells: &[CellConfig],
        clock_start: &str,
        fixtures: &[Fixture],
        settings: &BTreeMap<String, Value>,
    ) -> WorldDefinition {
        WorldDefinition {
            clock: Clock {
                start: date(clock_start),
            },
            cells: cells.to_vec(),
            settings: settings.clone(),
            fixtures: fixtures.to_vec(),
            actions: Vec::new(),
            deadlines: Vec::new(),
        }
    }

    fn toeslagen() -> Vec<CellConfig> {
        let config = serde_yaml_ng::from_str(
            r"
id: toeslagen
laws:
  - algemene_wet_inkomensafhankelijke_regelingen
chronicles:
  - stream: relaties
    key: bsn
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
        )
        .unwrap_or_else(|e| panic!("testconfig moet parsen: {e}"));
        vec![config]
    }

    fn fixture(at: &str, chronicle: &str, partnerschap: &str) -> Fixture {
        Fixture {
            at: date(at),
            record: Recording {
                cell: "toeslagen".to_string(),
                chronicle: chronicle.to_string(),
                name: "relatie_gewijzigd".to_string(),
                intake: Intake::Levering,
                grondslag: "AWIR art. 3".to_string(),
                fields: BTreeMap::from([
                    ("bsn".to_string(), Value::String("999993653".to_string())),
                    (
                        "partnerschap_type".to_string(),
                        Value::String(partnerschap.to_string()),
                    ),
                ]),
            },
        }
    }

    fn world(clock_start: &str, fixtures: &[Fixture]) -> World {
        World::from_definition(
            &definition(&toeslagen(), clock_start, fixtures, &no_settings()),
            &regulation_root(),
        )
        .unwrap_or_else(|e| panic!("wereld moet op te tuigen zijn: {e}"))
    }

    fn bsn() -> BTreeMap<String, Value> {
        BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))])
    }

    /// Een wereld zonder instellingen, voor elke test die geen verplichting kent.
    fn no_settings() -> BTreeMap<String, Value> {
        BTreeMap::new()
    }

    fn partner(world: &World, op_moment: &str) -> Value {
        world
            .reduce("toeslagen", "toeslagpartnerschap", &bsn(), date(op_moment))
            .unwrap_or_else(|e| panic!("de reductie op {op_moment} moet slagen: {e}"))
            .values()
            .unwrap_or_else(|| panic!("de reductie op {op_moment} hoort een antwoord te geven"))
            .get("heeft_toeslagpartner")
            .cloned()
            .unwrap_or_else(|| panic!("de reductie op {op_moment} hoort een antwoord te geven"))
    }

    #[test]
    fn fixture_van_voor_het_startmoment_staat_er_bij_het_optuigen_al() {
        let world = world(
            "2024-01-01",
            &[fixture("2023-01-01", "relaties", "HUWELIJK")],
        );
        assert_eq!(world.now(), date("2024-01-01"));
        assert_eq!(partner(&world, "2024-01-01"), Value::Bool(true));
    }

    #[test]
    fn een_latere_fixture_landt_pas_bij_advance_en_raakt_het_verleden_niet() {
        let mut world = world(
            "2024-01-01",
            &[
                fixture("2023-01-01", "relaties", "HUWELIJK"),
                fixture("2024-07-01", "relaties", "GEEN"),
            ],
        );
        assert_eq!(
            partner(&world, "2024-01-01"),
            Value::Bool(true),
            "een feit dat nog niet geland is, bestaat voor de cel niet"
        );

        world
            .advance(date("2025-01-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        assert_eq!(partner(&world, "2025-01-01"), Value::Bool(false));
        assert_eq!(
            partner(&world, "2024-01-01"),
            Value::Bool(true),
            "de vastlegging kreeg 2024-07-01 als moment, dus het beeld van \
             2024-01-01 blijft gelijk"
        );
    }

    /// Een trigger die nog moet afgaan mag de klok niet meenemen.
    ///
    /// Dit is de test die de lus vastpint in plaats van de tijdreductie. Elke
    /// andere assertie over de tijdlijn loopt via een reductie, en die filtert
    /// zelf al op `op_moment`, dus ze blijft ook groen als *elke* fixture
    /// meteen bij het optuigen wordt vastgelegd — de klok volledig negerend.
    /// Waar dat verschil wél zichtbaar wordt, is de stand van de klok: die
    /// bepaalt tot waar er gereduceerd mag worden, en een wereld die haar
    /// toekomst al heeft vastgelegd zou een antwoord geven op een moment
    /// waarover nog niets vaststaat.
    #[test]
    fn een_trigger_die_nog_moet_afgaan_zet_de_klok_niet_vooruit() {
        let world = world("2024-01-01", &[fixture("2024-07-01", "relaties", "GEEN")]);
        assert_eq!(
            world.now(),
            date("2024-01-01"),
            "de klok hoort op haar startmoment te staan, niet op dat van een \
             trigger die nog moet afgaan"
        );

        let err = world
            .reduce(
                "toeslagen",
                "toeslagpartnerschap",
                &bsn(),
                date("2024-07-01"),
            )
            .expect_err("het moment van een trigger die nog niet afging ligt ná de klok");
        assert!(
            matches!(err, SimulatorError::MomentAfterClock { .. }),
            "verwachtte MomentAfterClock, kreeg {err}"
        );
    }

    #[test]
    fn triggers_gaan_af_in_datumvolgorde_ongeacht_de_volgorde_in_het_bestand() {
        let mut world = world(
            "2024-01-01",
            &[
                fixture("2024-09-01", "relaties", "GEEN"),
                fixture("2024-03-01", "relaties", "HUWELIJK"),
            ],
        );
        world
            .advance(date("2025-01-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        assert_eq!(
            partner(&world, "2024-06-01"),
            Value::Bool(true),
            "tussen maart en september gold de vastlegging van maart"
        );
        assert_eq!(
            partner(&world, "2025-01-01"),
            Value::Bool(false),
            "na september geldt de vastlegging van september"
        );
    }

    #[test]
    fn een_moment_na_de_klok_wordt_geweigerd() {
        let world = world("2024-01-01", &[]);
        let err = world
            .reduce(
                "toeslagen",
                "toeslagpartnerschap",
                &bsn(),
                date("2024-06-01"),
            )
            .expect_err("een moment ná de klok hoort te falen");
        assert!(
            matches!(err, SimulatorError::MomentAfterClock { .. }),
            "verwachtte MomentAfterClock, kreeg {err}"
        );
    }

    #[test]
    fn de_klok_loopt_niet_terug() {
        let mut world = world("2024-06-01", &[]);
        let err = world
            .advance(date("2024-01-01"))
            .expect_err("achteruit lopen hoort te falen");
        assert!(
            matches!(err, SimulatorError::ClockRunsBackwards { .. }),
            "verwachtte ClockRunsBackwards, kreeg {err}"
        );
    }

    /// De executogram-vorm weigert wat ze niet kent, en dat geldt voor een
    /// veldnaam net zo goed als voor een kanaalnaam. Zonder die twee weigeringen
    /// staat er straks een vastlegging in een kroniek die iets anders zegt dan
    /// de auteur bedoelde: een grondslag die stil wegviel, of een kanaal
    /// waarvan niemand meer kan zeggen waarlangs het feit binnenkwam. Dat
    /// laatste is de hele reden dat `intake` een enum is en geen vrije tekst.
    #[test]
    fn een_onbekend_veld_of_kanaal_in_een_fixture_wordt_geweigerd() {
        let yaml = |grondslag: &str, intake: &str| {
            format!(
                r"
at: 2024-01-01
record:
  cell: toeslagen
  chronicle: relaties
  name: relatie_gewijzigd
  intake: {intake}
  {grondslag}: AWIR art. 3
  fields:
    bsn: '999993653'
"
            )
        };

        serde_yaml_ng::from_str::<Fixture>(&yaml("grondslag", "levering"))
            .unwrap_or_else(|e| panic!("een correcte fixture moet parsen: {e}"));
        assert!(
            serde_yaml_ng::from_str::<Fixture>(&yaml("grondlsag", "levering")).is_err(),
            "een typfout in een veldnaam hoort te falen; anders verdwijnt de grondslag stil"
        );
        assert!(
            serde_yaml_ng::from_str::<Fixture>(&yaml("grondslag", "leverng")).is_err(),
            "een typfout in een kanaalnaam hoort te falen"
        );
    }

    /// Twee triggers op dezelfde dag vallen op de tijdas niet uit elkaar; dan
    /// beslist de volgorde in het bestand, en de laatste wint. De sortering van
    /// de trigger-lijst is stabiel, dus dat is een vastgelegde eigenschap en
    /// geen toeval — zonder deze test zou een sortering die dat omgooit
    /// ongemerkt door kunnen.
    #[test]
    fn bij_een_gelijke_datum_beslist_de_volgorde_in_het_bestand() {
        let mut world = world(
            "2024-01-01",
            &[
                fixture("2024-06-01", "relaties", "HUWELIJK"),
                fixture("2024-06-01", "relaties", "GEEN"),
            ],
        );
        world
            .advance(date("2024-07-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        assert_eq!(
            partner(&world, "2024-07-01"),
            Value::Bool(false),
            "de laatste van twee triggers op dezelfde dag hoort te winnen"
        );
    }

    #[test]
    fn een_fixture_op_het_startmoment_staat_er_bij_het_optuigen_al() {
        let world = world(
            "2024-01-01",
            &[fixture("2024-01-01", "relaties", "HUWELIJK")],
        );
        assert_eq!(
            partner(&world, "2024-01-01"),
            Value::Bool(true),
            "de grens is inclusief: wat op het startmoment gebeurde, is gebeurd, \
             en een reductie op dat moment hoort het te zien"
        );
    }

    #[test]
    fn een_fixture_naar_een_onbekende_stroom_faalt_bij_het_optuigen() {
        let err = World::from_definition(
            &definition(
                &toeslagen(),
                "2024-01-01",
                &[fixture("2030-01-01", "betalingen", "GEEN")],
                &no_settings(),
            ),
            &regulation_root(),
        )
        .expect_err("een stroom die de cel niet houdt hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownChronicleStream { .. }),
            "verwachtte UnknownChronicleStream, kreeg {err}"
        );
    }

    /// De stroom met decretogrammen is niet met een `fixture` te vullen.
    ///
    /// Dat de configuratie haar niet mag declareren is niet genoeg: zodra een cel
    /// besluit-definities heeft, bestáát de stroom, en zonder deze poort zou een
    /// wereldbestand er een "besluit" in kunnen zetten dat nooit langs een engine
    /// kwam — zonder receipt, zonder herkomst, met een `intake` naar keuze. Een
    /// reductie erover zou dat niet van een echt besluit kunnen onderscheiden, en
    /// dan bewijst het kernscenario niets meer.
    #[test]
    fn een_fixture_kan_geen_decretogram_verzinnen() {
        let config: CellConfig = serde_yaml_ng::from_str(
            r"
id: toeslagen
laws:
  - wet_op_de_zorgtoeslag
  - algemene_wet_inkomensafhankelijke_regelingen
  - regeling_standaardpremie
besluit_definitions:
  - name: zorgtoeslag_vaststelling
    regulation: wet_op_de_zorgtoeslag
    output: heeft_recht_op_zorgtoeslag
    zaakkenmerk: 'zorgtoeslag/{bsn}'
    params:
      - name: bsn
        type: string
",
        )
        .unwrap_or_else(|e| panic!("testconfig moet parsen: {e}"));

        let verzonnen = Fixture {
            at: date("2024-06-01"),
            record: Recording {
                cell: "toeslagen".to_string(),
                chronicle: crate::cell::BESCHIKKINGEN.to_string(),
                name: "zorgtoeslag_vaststelling".to_string(),
                intake: Intake::Levering,
                grondslag: String::new(),
                fields: BTreeMap::from([
                    (
                        "zaakkenmerk".to_string(),
                        Value::String("zorgtoeslag/999993653".to_string()),
                    ),
                    ("heeft_recht_op_zorgtoeslag".to_string(), Value::Bool(true)),
                ]),
            },
        };

        let err = World::from_definition(
            &definition(&[config], "2024-01-01", &[verzonnen], &no_settings()),
            &regulation_root(),
        )
        .expect_err("een verzonnen decretogram hoort te falen");
        assert!(
            matches!(err, SimulatorError::ReservedStreamRecording { .. }),
            "verwachtte ReservedStreamRecording, kreeg {err}"
        );
    }

    #[test]
    fn een_fixture_naar_een_onbekende_cel_faalt_bij_het_optuigen() {
        let mut elders = fixture("2030-01-01", "relaties", "GEEN");
        elders.record.cell = "belastingdienst".to_string();
        let err = World::from_definition(
            &definition(&toeslagen(), "2024-01-01", &[elders], &no_settings()),
            &regulation_root(),
        )
        .expect_err("een cel die de wereld niet kent hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownCell { .. }),
            "verwachtte UnknownCell, kreeg {err}"
        );
    }

    /// Een fixture die het sleutelveld van haar stroom mist, hoort bij het
    /// optuigen te falen en niet halverwege de tijdlijn — en dat mag niet
    /// afhangen van de datum. Stond die datum vóór het startmoment, dan viel de
    /// fout op omdat de vastlegging bij het optuigen afging; lag hij erna, dan
    /// kwam dezelfde typfout pas bij `advance` boven water.
    #[test]
    fn een_fixture_zonder_sleutelveld_faalt_bij_het_optuigen_ook_als_haar_datum_ver_weg_ligt() {
        for at in ["2023-01-01", "2030-01-01"] {
            let mut zonder_sleutel = fixture(at, "relaties", "GEEN");
            zonder_sleutel.record.fields.remove("bsn");
            let err = World::from_definition(
                &definition(
                    &toeslagen(),
                    "2024-01-01",
                    &[zonder_sleutel],
                    &no_settings(),
                ),
                &regulation_root(),
            )
            .expect_err("een vastlegging zonder sleutelveld hoort te falen");
            assert!(
                matches!(err, SimulatorError::ChronicleEventWithoutKey { .. }),
                "fixture van {at}: verwachtte ChronicleEventWithoutKey, kreeg {err}"
            );
        }
    }

    /// Een kroniekfilter op een veld dat alleen via `fixtures` in een leeg
    /// opgetuigde stroom komt, hoort niet als typfout afgekeurd te worden.
    ///
    /// `declared_fields()` kent alleen wat in `chronicles[].events` van de
    /// celconfiguratie staat; een stroom die daar leeg blijft en haar inhoud
    /// pas via `fixtures` krijgt (zoals `scenarios/toeslagen_tijdlijn.yaml`)
    /// zou zonder deze toets een kroniekfilter op `partnerschap_type` ten
    /// onrechte als `UnknownFilterField` weigeren.
    #[test]
    fn een_kroniekfilter_op_een_veld_dat_alleen_via_fixtures_komt_wordt_niet_afgekeurd() {
        let config: CellConfig = serde_yaml_ng::from_str(
            r"
id: toeslagen
laws: []
chronicles:
  - stream: relaties
    key: bsn
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
        .unwrap_or_else(|e| panic!("testconfig moet parsen: {e}"));

        let world = World::from_definition(
            &definition(
                &[config],
                "2024-01-01",
                &[fixture("2023-01-01", "relaties", "HUWELIJK")],
                &no_settings(),
            ),
            &regulation_root(),
        )
        .unwrap_or_else(|e| panic!("een wereld met dit kroniekfilter moet op te tuigen zijn: {e}"));

        let answer = world
            .reduce("toeslagen", "partnerschap", &bsn(), date("2024-01-01"))
            .unwrap_or_else(|e| panic!("de reductie moet slagen: {e}"));
        assert_eq!(
            answer
                .values()
                .unwrap_or_else(|| panic!("verwachtte een vastgesteld feit"))
                .get("partnerschap_type"),
            Some(&Value::String("HUWELIJK".to_string())),
            "de fixture-waarde hoort in het antwoord van het kroniekfilter"
        );
    }

    /// Een wereld waarin één besluit een verplichting oplegt.
    ///
    /// `schedule` wordt letterlijk ingeplakt, zodat elke test hieronder alleen
    /// varieert waar ze over gaat. De betalende cel is een bron-cel zonder
    /// wetten: een verplichting nakomen vraagt geen engine.
    fn verplichting_configs(schedule: &str, betaler_houdt_betalingen: bool) -> Vec<CellConfig> {
        // Zonder stroom valt er ook niets over te reduceren: de lexostatus gaat
        // dan mee weg, anders struikelt de cel over haar eigen definitie voordat
        // de wereld aan de verplichting toekomt.
        let betaler_streams = if betaler_houdt_betalingen {
            "chronicles:
  - stream: betalingen
    key: zaakkenmerk
lexostatus_definitions:
  - name: betaald
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
        } else {
            "chronicles: []\n"
        };
        let parse = |yaml: &str| -> CellConfig {
            serde_yaml_ng::from_str(yaml)
                .unwrap_or_else(|e| panic!("testconfig moet parsen: {e}\n{yaml}"))
        };
        vec![
            parse(&format!(
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
        op_moment: 2023-11-15
        fields:
          bsn: '999993653'
          partnerschap_type: GEEN
          is_verzekerde: true
          verzamelinkomen: 79547
          buitenlands_inkomen: 0
          vermogen: 0
  - stream: betalingen
    key: zaakkenmerk
besluit_definitions:
  - name: zorgtoeslag_vaststelling
    regulation: wet_op_de_zorgtoeslag
    output: heeft_recht_op_zorgtoeslag
    outputs:
      - hoogte_zorgtoeslag
    zaakkenmerk: 'zorgtoeslag/{{bsn}}'
    params:
      - name: bsn
        type: string
    inputs:
      bsn:
        param: bsn
      is_verzekerde:
        from_chronicle: inkomensleveringen
        field: is_verzekerde
    obligations:
{schedule}
lexostatus_definitions:
  - name: betaald
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
            )),
            parse(&format!(
                r"
id: belastingdienst
laws: []
{betaler_streams}"
            )),
        ]
    }

    /// De gewone verplichting van de tests hieronder: vier kwartaaltermijnen.
    const KWARTAAL: &str = "      - amount: $hoogte_zorgtoeslag
        payer: belastingdienst
        schedule: kwartaal";

    fn verplichting_wereld(schedule: &str, settings: &BTreeMap<String, Value>) -> Result<World> {
        World::from_definition(
            &definition(
                &verplichting_configs(schedule, true),
                "2024-01-01",
                &[],
                settings,
            ),
            &regulation_root(),
        )
    }

    /// Wat er op `op_moment` betaald is volgens deze cel; `None` is "niets
    /// vastgesteld".
    fn betaald(world: &World, cell: &str, op_moment: &str) -> Option<Value> {
        let params = BTreeMap::from([(
            "zaakkenmerk".to_string(),
            Value::String("zorgtoeslag/999993653".to_string()),
        )]);
        world
            .reduce(cell, "betaald", &params, date(op_moment))
            .unwrap_or_else(|e| panic!("de som moet te maken zijn: {e}"))
            .values()
            .and_then(|values| values.get("bedrag").cloned())
    }

    fn beslis(world: &mut World) -> Decretogram {
        world
            .decide(
                "toeslagen",
                "zorgtoeslag_vaststelling",
                &bsn(),
                date("2024-01-01"),
            )
            .unwrap_or_else(|e| panic!("het besluit moet genomen kunnen worden: {e}"))
            .decretogram
    }

    /// De eerste termijn vervalt op het moment van het besluit, dus ze wordt
    /// meteen nagekomen. Zou ze blijven wachten tot de volgende `advance`, dan
    /// werd ze dáár met terugwerkende kracht vastgelegd — en dan verandert het
    /// beeld van een moment dat al geweest is.
    #[test]
    fn de_eerste_termijn_gaat_af_op_het_moment_van_het_besluit() {
        let mut world = verplichting_wereld(KWARTAAL, &no_settings())
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
        let gram = beslis(&mut world);

        assert_eq!(gram.obligations.len(), 4, "kwartaal kent vier termijnen");
        assert_eq!(gram.obligations[0].vervaldatum, date("2024-01-01"));
        assert_eq!(
            betaald(&world, "belastingdienst", "2024-01-01"),
            Some(Value::Int(49301))
        );
        assert_eq!(
            betaald(&world, "toeslagen", "2024-01-01"),
            Some(Value::Int(49301)),
            "de besluitende cel legt vast dat het haar gemeld is, in haar eigen kroniek"
        );
    }

    /// De klok mag in zo kleine stappen langskomen als ze wil: elke termijn
    /// wordt één keer nagekomen. Zonder deze test zou een `advance` die een
    /// vervaldatum twee keer passeert er twee betalingen van maken, en dan telt
    /// de som dubbel.
    #[test]
    fn kleine_stappen_leveren_niet_meer_betalingen_op() {
        let mut world = verplichting_wereld(KWARTAAL, &no_settings())
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
        beslis(&mut world);

        let mut dag = date("2024-01-01");
        let eind = date("2024-12-31");
        while dag <= eind {
            world
                .advance(dag)
                .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
            dag = dag
                .succ_opt()
                .unwrap_or_else(|| panic!("{dag} moet een volgende dag hebben"));
        }

        assert_eq!(
            betaald(&world, "belastingdienst", "2024-12-31"),
            Some(Value::Decimal("197205.31187".parse().unwrap_or_default())),
            "vier termijnen, precies het toegekende bedrag, ook na 366 stappen"
        );
    }

    /// Eén besluit mag meer dan één verplichting opleggen, en dan hoort elke
    /// termijn van elk van beide nagekomen te worden.
    ///
    /// Zonder doorlopende volgnummers begint elke verplichting weer bij 1, en dan
    /// gaat de eerste termijn van de tweede verplichting door voor die van de
    /// eerste: hij wordt als "al betaald" overgeslagen en valt stil weg. De som
    /// is het enige waar dat aan te zien is.
    #[test]
    fn twee_verplichtingen_in_een_besluit_worden_beide_nagekomen() {
        let twee = "      - amount: $hoogte_zorgtoeslag
        payer: belastingdienst
        schedule: kwartaal
      - amount: $hoogte_zorgtoeslag
        payer: belastingdienst
        schedule: ineens";
        let mut world = verplichting_wereld(twee, &no_settings())
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));

        let gram = beslis(&mut world);
        assert_eq!(
            gram.obligations.len(),
            5,
            "vier kwartaaltermijnen plus één ineens"
        );
        let volgnummers: Vec<i64> = gram
            .obligations
            .iter()
            .map(|due| due.volgnummer)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(
            volgnummers,
            vec![1, 2, 3, 4, 5],
            "de volgnummers van één gram horen uniek te zijn"
        );

        world
            .advance(date("2025-01-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
        assert_eq!(
            betaald(&world, "belastingdienst", "2025-01-01"),
            Some(Value::Decimal("394410.62374".parse().unwrap_or_default())),
            "beide verplichtingen zijn opgelegd, dus beide horen betaald te zijn"
        );
        assert_eq!(
            betaald(&world, "toeslagen", "2025-01-01"),
            Some(Value::Decimal("394410.62374".parse().unwrap_or_default())),
            "en de besluitende cel hoort van beide de melding te hebben"
        );
    }

    /// Over één zaak worden meer besluiten genomen, elk met een eigen schema dat
    /// bij termijn 1 begint. Dat tweede schema hoort óók nagekomen te worden.
    ///
    /// Herkende de idempotentie een termijn op zaak en volgnummer alleen, dan zou
    /// de eerste termijn van het tweede besluit voor die van het eerste doorgaan
    /// — precies het geval van een verlening met daarna een vaststelling.
    #[test]
    fn een_tweede_besluit_over_dezelfde_zaak_wordt_ook_nagekomen() {
        let mut configs = verplichting_configs(KWARTAAL, true);
        let mut tweede = configs[0].besluit_definitions[0].clone();
        tweede.name = "zorgtoeslag_herziening".to_string();
        tweede.obligations = vec![ObligationDefinition {
            amount: "$hoogte_zorgtoeslag".to_string(),
            payer: "belastingdienst".to_string(),
            schedule: "ineens".to_string(),
            from: None,
        }];
        configs[0].besluit_definitions.push(tweede);

        let mut world = World::from_definition(
            &definition(&configs, "2024-01-01", &[], &no_settings()),
            &regulation_root(),
        )
        .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));

        beslis(&mut world);
        world
            .decide(
                "toeslagen",
                "zorgtoeslag_herziening",
                &bsn(),
                date("2024-01-01"),
            )
            .unwrap_or_else(|e| panic!("het tweede besluit moet genomen kunnen worden: {e}"));
        world
            .advance(date("2025-01-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        assert_eq!(
            betaald(&world, "belastingdienst", "2025-01-01"),
            Some(Value::Decimal("394410.62374".parse().unwrap_or_default())),
            "twee besluiten, twee schema's, twee keer het toegekende bedrag"
        );
    }

    /// Twee keer hetzelfde besluit op dezelfde dag plant twee keer hetzelfde
    /// schema. Nagekomen wordt het één keer: dezelfde zaak, hetzelfde besluit en
    /// hetzelfde volgnummer is dezelfde termijn.
    #[test]
    fn een_tweede_besluit_op_dezelfde_dag_levert_geen_tweede_betaling() {
        let mut world = verplichting_wereld(KWARTAAL, &no_settings())
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
        beslis(&mut world);
        beslis(&mut world);
        world
            .advance(date("2025-01-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        assert_eq!(
            betaald(&world, "belastingdienst", "2025-01-01"),
            Some(Value::Decimal("197205.31187".parse().unwrap_or_default())),
            "het bedrag is één keer toegekend, dus het wordt één keer betaald"
        );
    }

    /// Een verplichting die tijdens de run ingepland wordt, moet vóór een al
    /// wachtende trigger met een latere datum afgaan.
    ///
    /// Dit is de invariant van de wachtrij, en ze is stil te breken: wie een
    /// nieuwe trigger achteraan zet, ziet drie van de vier betalingen gewoon
    /// gebeuren. Pas als er iets anders ná hen staat, blijkt dat ze te laat of
    /// helemaal niet afgaan.
    #[test]
    fn een_verplichting_wordt_op_datumpositie_ingepland() {
        let laat = Fixture {
            at: date("2030-01-01"),
            record: Recording {
                cell: "toeslagen".to_string(),
                chronicle: "inkomensleveringen".to_string(),
                name: "inkomenslevering".to_string(),
                intake: Intake::Levering,
                grondslag: String::new(),
                fields: BTreeMap::from([(
                    "bsn".to_string(),
                    Value::String("999993653".to_string()),
                )]),
            },
        };
        let mut world = World::from_definition(
            &definition(
                &verplichting_configs(KWARTAAL, true),
                "2024-01-01",
                &[laat],
                &no_settings(),
            ),
            &regulation_root(),
        )
        .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));

        beslis(&mut world);
        world
            .advance(date("2025-01-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        assert_eq!(
            betaald(&world, "belastingdienst", "2025-01-01"),
            Some(Value::Decimal("197205.31187".parse().unwrap_or_default())),
            "alle vier de termijnen horen af te gaan, ook met een trigger van 2030 in de rij"
        );
        assert_eq!(
            world.pending_triggers(),
            1,
            "alleen de fixture van 2030 hoort nog te wachten"
        );
    }

    /// Het ritme mag een instelling van de wereld zijn; een instelling die niet
    /// bestaat hoort bij het optuigen te vallen en niet bij het besluit dat erop
    /// leunt.
    #[test]
    fn een_ritme_uit_de_instellingen_wordt_gelezen_en_een_onbekende_geweigerd() {
        let uit_instelling = "      - amount: $hoogte_zorgtoeslag
        payer: belastingdienst
        schedule: $betalingsritme";

        let settings = BTreeMap::from([(
            "betalingsritme".to_string(),
            Value::String("maand".to_string()),
        )]);
        let mut world = verplichting_wereld(uit_instelling, &settings)
            .unwrap_or_else(|e| panic!("een ritme uit de instellingen moet werken: {e}"));
        assert_eq!(
            beslis(&mut world).obligations.len(),
            12,
            "de instelling zegt 'maand', dus twaalf termijnen"
        );

        let err = verplichting_wereld(uit_instelling, &no_settings())
            .expect_err("een instelling die niet bestaat hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownSetting { .. }),
            "verwachtte UnknownSetting, kreeg {err}"
        );
    }

    /// Een instelling die geen ritme is, is even fout als een typfout in de
    /// definitie zelf — en hoort op hetzelfde moment te blijken.
    #[test]
    fn een_instelling_die_geen_ritme_is_wordt_geweigerd() {
        let settings = BTreeMap::from([(
            "betalingsritme".to_string(),
            Value::String("per_week".to_string()),
        )]);
        let err = verplichting_wereld(
            "      - amount: $hoogte_zorgtoeslag
        payer: belastingdienst
        schedule: $betalingsritme",
            &settings,
        )
        .expect_err("een instelling die geen ritme noemt hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownSchedule { .. }),
            "verwachtte UnknownSchedule, kreeg {err}"
        );
    }

    /// Zonder betalingsstroom bij de betaler zou de eerste vervaldatum omvallen,
    /// halverwege de tijdlijn. Dat hoort bij het optuigen te blijken.
    #[test]
    fn een_betaler_zonder_betalingsstroom_faalt_bij_het_optuigen() {
        let err = World::from_definition(
            &definition(
                &verplichting_configs(KWARTAAL, false),
                "2024-01-01",
                &[],
                &no_settings(),
            ),
            &regulation_root(),
        )
        .expect_err("een betaler zonder betalingsstroom hoort te falen");
        assert!(
            matches!(err, SimulatorError::ObligationStream { .. }),
            "verwachtte ObligationStream, kreeg {err}"
        );
    }

    /// De betalende cel moet bestaan. Een typfout in `payer` zou anders een
    /// verplichting opleveren die aan niemand is opgelegd.
    #[test]
    fn een_onbekende_betaler_faalt_bij_het_optuigen() {
        let err = verplichting_wereld(
            "      - amount: $hoogte_zorgtoeslag
        payer: betaalsysteem
        schedule: kwartaal",
            &no_settings(),
        )
        .expect_err("een betaler die de wereld niet kent hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownCell { .. }),
            "verwachtte UnknownCell, kreeg {err}"
        );
    }

    /// Een `from` vóór het besluit zou een betaling op een moment vastleggen dat
    /// al geweest is, en dan verandert het beeld van toen alsnog.
    #[test]
    fn een_verplichting_die_voor_het_besluit_vervalt_wordt_geweigerd() {
        let mut world = verplichting_wereld(
            "      - amount: $hoogte_zorgtoeslag
        payer: belastingdienst
        schedule: kwartaal
        from: '2023-01-01'",
            &no_settings(),
        )
        .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));

        let err = world
            .decide(
                "toeslagen",
                "zorgtoeslag_vaststelling",
                &bsn(),
                date("2024-01-01"),
            )
            .expect_err("een termijn vóór het besluit hoort te falen");
        assert!(
            matches!(err, SimulatorError::ObligationBeforeDecision { .. }),
            "verwachtte ObligationBeforeDecision, kreeg {err}"
        );
    }

    /// Een wereldbestand is los te lezen, en het weigert een typfout.
    ///
    /// Los, want een web-laag leest één wereldbestand en maakt er per sessie een
    /// verse wereld uit; een scenario zet er alleen stappen bovenop. Weigert het
    /// een typfout niet, dan valt een onbekend veld stil weg en doet een actie of
    /// een termijn niets zonder dat iemand het merkt.
    #[test]
    fn een_wereldbestand_is_los_te_lezen_en_weigert_een_typfout() {
        let yaml = |sleutel: &str| {
            format!(
                r"
clock:
  start: 2024-01-01
cells:
  - id: burger
    laws: []
    chronicles:
      - stream: aanvragen
        key: bsn
{sleutel}:
  - label: aanvraag ontvangen vóór 1 maart
    at: 2024-03-01
    warn_if_missing:
      cell: burger
      chronicle: aanvragen
      name: aanvraag_ingediend
"
            )
        };

        let definition = WorldDefinition::from_yaml(&yaml("deadlines"))
            .unwrap_or_else(|e| panic!("een wereldbestand moet los te lezen zijn: {e}"));
        assert_eq!(definition.deadlines.len(), 1);
        World::from_definition(&definition, &regulation_root())
            .unwrap_or_else(|e| panic!("en op te tuigen: {e}"));

        assert!(
            WorldDefinition::from_yaml(&yaml("deadlnies")).is_err(),
            "een typfout in een sleutel hoort te falen; anders doet de termijn niets"
        );
    }

    /// Een termijn die naar een stroom wijst die de cel niet houdt, zou altijd
    /// waarschuwen — over een feit dat nergens kon liggen. Dat hoort bij het
    /// optuigen te vallen.
    #[test]
    fn een_termijn_naar_een_onbekende_stroom_faalt_bij_het_optuigen() {
        let mut spec = definition(&actie_cellen(), "2024-01-01", &[], &no_settings());
        spec.actions = vec![aanvraag_actie("")];
        spec.deadlines = vec![Deadline {
            label: "aanvraag op tijd".to_string(),
            at: date("2024-03-01"),
            warn_if_missing: ExpectedFact {
                cell: "toeslagen".to_string(),
                chronicle: "verantwoordingen".to_string(),
                name: "verantwoording_ontvangen".to_string(),
            },
        }];
        let err = World::from_definition(&spec, &regulation_root())
            .expect_err("een stroom die de cel niet houdt hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownStream { .. }),
            "verwachtte UnknownStream, kreeg {err}"
        );
    }

    /// Een vraag aan een cel die niet bestaat, over een moment dat nog niet
    /// geweest is.
    ///
    /// Twee dingen zijn er mis en er kan er maar één gemeld worden; het hoort de
    /// cel te zijn. Andersom komt de onbekende cel in een melding over de klok te
    /// staan — "cel 'belastingdienst' … ligt ná de klok" — en dan zoekt de lezer
    /// een klokprobleem bij een cel die er niet is.
    #[test]
    fn een_onbekende_cel_gaat_voor_het_moment() {
        let world = world("2024-01-01", &[]);
        let err = world
            .reduce(
                "belastingdienst",
                "toeslagpartnerschap",
                &bsn(),
                date("2024-06-01"),
            )
            .expect_err("een cel die de wereld niet kent hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownCell { .. }),
            "verwachtte UnknownCell, kreeg {err}"
        );
    }

    /// Twee bron-cellen en één actie: de aanvrager legt vast en levert.
    ///
    /// Bron-cellen, want een actie vraagt geen engine — en dan gaat deze
    /// testgroep over wat een actie doet en niet over wat een wet uitrekent.
    fn actie_cellen() -> Vec<CellConfig> {
        let parse = |yaml: &str| -> CellConfig {
            serde_yaml_ng::from_str(yaml)
                .unwrap_or_else(|e| panic!("testconfig moet parsen: {e}\n{yaml}"))
        };
        vec![
            parse(
                r"
id: burger
laws: []
chronicles:
  - stream: aanvragen
    key: bsn
",
            ),
            parse(
                r"
id: toeslagen
laws: []
chronicles:
  - stream: aanvragen
    key: bsn
lexostatus_definitions:
  - name: ontvangen_aanvraag
    inputs:
      - name: bsn
        type: string
    outputs:
      - jaar
    reduction:
      chronicle: aanvragen
      key: bsn
      latest: true
",
            ),
        ]
    }

    /// De aanvraag-actie, met of zonder voorwaarde.
    fn aanvraag_actie(available_when: &str) -> ActionDefinition {
        let yaml = format!(
            r"
id: burger.aanvraag
actor: burger
label: Aanvraag indienen
records:
  cell: burger
  chronicle: aanvragen
  name: aanvraag_ingediend
  intake: aanvraag
  fields:
    - name: bsn
      type: string
    - name: jaar
      type: number
  delivers_to:
    cell: toeslagen
    chronicle: aanvragen
    name: aanvraag_ontvangen
    intake: aanvraag
{available_when}"
        );
        serde_yaml_ng::from_str(&yaml)
            .unwrap_or_else(|e| panic!("testactie moet parsen: {e}\n{yaml}"))
    }

    /// Een wereld met die ene actie erin.
    fn actie_wereld(available_when: &str) -> Result<World> {
        let mut spec = definition(&actie_cellen(), "2024-01-01", &[], &no_settings());
        spec.actions = vec![aanvraag_actie(available_when)];
        World::from_definition(&spec, &regulation_root())
    }

    /// Het ingevulde formulier van de aanvraag-actie.
    fn aanvraag_waarden() -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("bsn".to_string(), Value::String("999993653".to_string())),
            ("jaar".to_string(), Value::Int(2024)),
        ])
    }

    /// Een actie met `delivers_to` legt hetzelfde feit twee keer vast: bij de
    /// actor en bij de ontvanger, elk in een eigen kroniek met een eigen kanaal.
    ///
    /// Dat zijn twee grammen en niet één gedeeld gram. Zou er maar één komen, dan
    /// zou één van de twee cellen iets moeten weten uit de kroniek van de ander —
    /// en dat is precies wat deze opstelling onmogelijk hoort te maken.
    #[test]
    fn een_actie_legt_vast_bij_de_actor_en_levert_bij_de_ontvanger() {
        let mut world =
            actie_wereld("").unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
        let events = world
            .act("burger.aanvraag", &aanvraag_waarden())
            .unwrap_or_else(|e| panic!("de actie moet kunnen: {e}"));

        assert_eq!(events.recordings.len(), 2, "twee grammen, twee kronieken");
        assert_eq!(events.recordings[0].cell, "burger");
        assert_eq!(events.recordings[1].cell, "toeslagen");
        assert_eq!(
            events.recordings[1].event.recording_actor, "toeslagen",
            "de ontvanger legt op eigen naam vast, niet op naam van de aanvrager"
        );
        assert_eq!(
            events.recordings[1].event.name, "aanvraag_ontvangen",
            "bij de ontvanger heet het gram wat het wereldbestand zegt"
        );
        assert_eq!(
            events.recordings[0].event.op_moment,
            date("2024-01-01"),
            "een actie gebeurt op de stand van de klok en niet op een eigen datum"
        );

        let params = BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))]);
        let answer = world
            .reduce(
                "toeslagen",
                "ontvangen_aanvraag",
                &params,
                date("2024-01-01"),
            )
            .unwrap_or_else(|e| panic!("de reductie moet slagen: {e}"));
        assert_eq!(
            answer
                .values()
                .and_then(|values| values.get("jaar"))
                .cloned(),
            Some(Value::Int(2024)),
            "de geleverde waarde hoort bij de ontvanger terug te vinden te zijn"
        );
    }

    /// Een actie die op een feit wacht, weigert leesbaar zolang dat feit er niet
    /// is — en kan zodra het er is.
    #[test]
    fn een_actie_met_een_voorwaarde_kan_pas_als_het_feit_er_ligt() {
        let voorwaarde = "available_when:
  cell: toeslagen
  chronicle: aanvragen
  field: jaar
  equals: 2024
";
        let mut world = actie_wereld(voorwaarde)
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));

        let err = world
            .act("burger.aanvraag", &aanvraag_waarden())
            .expect_err("zonder het feit hoort de actie te weigeren");
        let SimulatorError::ActionNotAvailable { reason, .. } = &err else {
            panic!("verwachtte ActionNotAvailable, kreeg {err}");
        };
        assert!(
            reason.contains("aanvragen") && reason.contains("jaar"),
            "de weigering hoort te zeggen wat er nog niet ligt, kreeg: {reason}"
        );
        assert!(
            !world
                .snapshot()
                .actions
                .iter()
                .any(|action| action.available),
            "en het beeld hoort de actie als nog-niet-mogelijk te tonen"
        );
    }

    /// Het formulier van een actie is een belofte, en dus even streng als de
    /// gedocumenteerde parameters van een lexostatus: precies de velden die er
    /// staan, van het type dat er staat.
    #[test]
    fn een_formulier_weigert_een_veld_dat_het_niet_documenteert() {
        let mut world =
            actie_wereld("").unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));

        let mut erbij = aanvraag_waarden();
        erbij.insert("toetsingsinkomen".to_string(), Value::Int(81000));
        let err = world
            .act("burger.aanvraag", &erbij)
            .expect_err("een veld dat het formulier niet kent hoort te falen");
        assert!(
            matches!(err, SimulatorError::UndocumentedParameter { .. }),
            "verwachtte UndocumentedParameter, kreeg {err}"
        );

        let mut verkeerd_type = aanvraag_waarden();
        verkeerd_type.insert("jaar".to_string(), Value::String("2024".to_string()));
        let err = world
            .act("burger.aanvraag", &verkeerd_type)
            .expect_err("een waarde van het verkeerde type hoort te falen");
        assert!(
            matches!(err, SimulatorError::ParameterType { .. }),
            "verwachtte ParameterType, kreeg {err}"
        );
    }

    /// Een actie die vastlegt in een stroom die het sleutelveld niet uit haar
    /// formulier kan krijgen, hoort bij het optuigen te vallen.
    ///
    /// Anders is de actie een knop die altijd stuk gaat, en dat merkt iemand pas
    /// als hij erop drukt.
    #[test]
    fn een_actie_zonder_sleutelveld_in_haar_formulier_faalt_bij_het_optuigen() {
        let mut actie = aanvraag_actie("");
        let ActionEffect::Records(records) = &mut actie.effect else {
            panic!("de testactie legt vast");
        };
        records.fields.retain(|field| field.name != "bsn");

        let mut spec = definition(&actie_cellen(), "2024-01-01", &[], &no_settings());
        spec.actions = vec![actie];
        let err = World::from_definition(&spec, &regulation_root())
            .expect_err("een formulier zonder het sleutelveld hoort te falen");
        let SimulatorError::ActionRecording { reason, .. } = &err else {
            panic!("verwachtte ActionRecording, kreeg {err}");
        };
        assert!(
            reason.contains("bsn"),
            "de melding hoort het sleutelveld te noemen, kreeg: {reason}"
        );
    }

    /// Aan jezelf leveren wat je net zelf vastlegde, is geen tweede feit.
    #[test]
    fn een_levering_aan_de_eigen_cel_wordt_geweigerd() {
        let mut actie = aanvraag_actie("");
        let ActionEffect::Records(records) = &mut actie.effect else {
            panic!("de testactie legt vast");
        };
        // Onbereikbaar leeg: de testactie levert. De actor legt hier vast in de
        // kroniek van de ontvanger, zodat de twee kanten samenvallen zonder dat de
        // lexostatus van de ontvanger haar veld kwijtraakt.
        if let Some(delivery) = &mut records.delivers_to {
            records.cell = delivery.cell.clone();
        }

        let mut spec = definition(&actie_cellen(), "2024-01-01", &[], &no_settings());
        spec.actions = vec![actie];
        let err = World::from_definition(&spec, &regulation_root())
            .expect_err("leveren aan jezelf hoort te falen");
        assert!(
            matches!(err, SimulatorError::DeliveryToSelf { .. }),
            "verwachtte DeliveryToSelf, kreeg {err}"
        );
    }

    /// Twee acties met hetzelfde id: de tweede zou de eerste stil schaduwen, want
    /// een actie wordt op haar id aangeroepen.
    #[test]
    fn twee_acties_met_hetzelfde_id_worden_geweigerd() {
        let mut spec = definition(&actie_cellen(), "2024-01-01", &[], &no_settings());
        spec.actions = vec![aanvraag_actie(""), aanvraag_actie("")];
        let err = World::from_definition(&spec, &regulation_root())
            .expect_err("twee acties met hetzelfde id horen te falen");
        assert!(
            matches!(err, SimulatorError::DuplicateAction { .. }),
            "verwachtte DuplicateAction, kreeg {err}"
        );
    }

    /// Een actie die de wereld niet kent, is een fout die opsomt wat er wél is.
    #[test]
    fn een_onbekende_actie_noemt_de_acties_die_er_wel_zijn() {
        let mut world =
            actie_wereld("").unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
        let err = world
            .act("burger.verantwoording", &aanvraag_waarden())
            .expect_err("een actie die er niet is hoort te falen");
        let SimulatorError::UnknownAction { known, .. } = &err else {
            panic!("verwachtte UnknownAction, kreeg {err}");
        };
        assert_eq!(known, "burger.aanvraag");
    }

    /// Een verplichting met `schedule: $betalingsritme`, voor de tests over
    /// instellingen.
    const RITME_UIT_INSTELLING: &str = "      - amount: $hoogte_zorgtoeslag
        payer: belastingdienst
        schedule: $betalingsritme";

    /// De instellingen van een wereld met één betalingsritme.
    fn ritme(naam: &str) -> BTreeMap<String, Value> {
        BTreeMap::from([(
            "betalingsritme".to_string(),
            Value::String(naam.to_string()),
        )])
    }

    /// Een instelling mag om zolang er geen besluit op leunt, en daarna niet meer.
    ///
    /// Het gram legt vast waarop besloten is. Een ritme dat er achteraf onder
    /// vandaan geschoven wordt, laat het gram iets anders zeggen dan er gebeurd
    /// is — en dan is een decretogram niet meer terug te lezen.
    #[test]
    fn een_instelling_die_een_besluit_gebruikte_staat_vast() {
        let mut world = verplichting_wereld(RITME_UIT_INSTELLING, &ritme("kwartaal"))
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));

        world
            .update_settings(&ritme("maand"))
            .unwrap_or_else(|e| panic!("vóór het eerste besluit mag het ritme om: {e}"));
        assert_eq!(
            beslis(&mut world).obligations.len(),
            12,
            "de gewijzigde instelling hoort het besluit te sturen"
        );

        let err = world
            .update_settings(&ritme("ineens"))
            .expect_err("na het besluit hoort de instelling vast te staan");
        let SimulatorError::SettingInUse {
            setting, besluit, ..
        } = &err
        else {
            panic!("verwachtte SettingInUse, kreeg {err}");
        };
        assert_eq!(setting, "betalingsritme");
        assert_eq!(besluit, "zorgtoeslag_vaststelling");
        assert!(
            world
                .snapshot()
                .locked_settings
                .contains_key("betalingsritme"),
            "en het beeld hoort te laten zien dat deze knop niet meer om kan"
        );
    }

    /// Een instelling die de wereld niet kent, wijzigen: dat is een knop die
    /// niets doet, en een typfout ziet er precies zo uit.
    #[test]
    fn een_onbekende_instelling_wijzigen_wordt_geweigerd() {
        let mut world = verplichting_wereld(RITME_UIT_INSTELLING, &ritme("kwartaal"))
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
        let wijziging = BTreeMap::from([(
            "betalingsrtime".to_string(),
            Value::String("maand".to_string()),
        )]);
        let err = world
            .update_settings(&wijziging)
            .expect_err("een instelling die niet bestaat hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownWorldSetting { .. }),
            "verwachtte UnknownWorldSetting, kreeg {err}"
        );
    }

    /// Een instelling die geen ritme is, hoort bij het wijzigen te vallen en niet
    /// bij het eerstvolgende besluit dat erop leunt.
    #[test]
    fn een_instelling_wijzigen_naar_iets_dat_geen_ritme_is_wordt_geweigerd() {
        let mut world = verplichting_wereld(RITME_UIT_INSTELLING, &ritme("kwartaal"))
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
        let err = world
            .update_settings(&ritme("per_week"))
            .expect_err("een ritme dat niet bestaat hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownSchedule { .. }),
            "verwachtte UnknownSchedule, kreeg {err}"
        );
        assert_eq!(
            world.snapshot().settings,
            ritme("kwartaal"),
            "een geweigerde wijziging hoort niets te veranderen"
        );
    }

    /// Terugzetten is opnieuw beginnen, niet terugdraaien.
    ///
    /// De klok staat weer op haar startmoment, de besluiten zijn er niet meer, en
    /// ook de instellingen zijn weer die van het bestand: wie ze wijzigde,
    /// wijzigde de wereld en niet het bestand.
    #[test]
    fn reset_zet_de_wereld_terug_naar_de_startstand() {
        let mut world = verplichting_wereld(RITME_UIT_INSTELLING, &ritme("kwartaal"))
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
        world
            .update_settings(&ritme("maand"))
            .unwrap_or_else(|e| panic!("het ritme mag hier nog om: {e}"));
        beslis(&mut world);
        world
            .advance(date("2024-12-31"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        world
            .reset()
            .unwrap_or_else(|e| panic!("de wereld moet terug te zetten zijn: {e}"));

        assert_eq!(
            world.now(),
            date("2024-01-01"),
            "de klok staat weer vooraan"
        );
        assert_eq!(
            world.snapshot().settings,
            ritme("kwartaal"),
            "de instellingen zijn weer die van het bestand"
        );
        assert!(
            world.snapshot().locked_settings.is_empty(),
            "en er leunt weer geen besluit op"
        );
        assert_eq!(
            betaald(&world, "belastingdienst", "2024-01-01"),
            None,
            "de betalingen van de vorige run zijn er niet meer"
        );
    }

    /// Het beeld van de wereld draagt geen receipt.
    ///
    /// Het receipt van RFC-013 draagt wandkloktijd, en een beeld dat per run
    /// verschilt is geen contract. Wat een lezer eraan had — de herkomst van elke
    /// waarde waarop besloten is — staat er wél, per veld.
    #[test]
    fn het_beeld_draagt_geen_receipt_en_wel_de_herkomst_per_waarde() {
        let mut world = verplichting_wereld(KWARTAAL, &no_settings())
            .unwrap_or_else(|e| panic!("de wereld moet op te tuigen zijn: {e}"));
        beslis(&mut world);

        let snapshot = world.snapshot();
        let grammen: Vec<&crate::snapshot::GramSnapshot> = snapshot
            .cells
            .iter()
            .flat_map(|cell| &cell.chronicles)
            .flat_map(|chronicle| &chronicle.grams)
            .filter(|gram| gram.kind == crate::snapshot::GramKind::Decretogram)
            .collect();
        assert_eq!(
            grammen.len(),
            1,
            "één besluit hoort één decretogram te zijn"
        );

        let gram = grammen[0];
        assert!(
            !gram.fields.contains_key(crate::cell::RECEIPT),
            "het receipt hoort niet in het beeld te staan; velden: {:?}",
            gram.fields.keys().collect::<Vec<_>>()
        );
        assert!(
            matches!(
                gram.fields.get("is_verzekerde").map(|field| &field.origin),
                Some(crate::snapshot::FieldOrigin::BesluitInput { .. })
            ),
            "een input hoort de herkomst te dragen die het gram vastlegde"
        );
        assert!(
            matches!(
                gram.fields
                    .get("hoogte_zorgtoeslag")
                    .map(|field| &field.origin),
                Some(crate::snapshot::FieldOrigin::Computed { .. })
            ),
            "een uitkomst hoort als berekend door de regeling te verschijnen"
        );
        assert!(
            matches!(
                gram.fields.get(ZAAKKENMERK).map(|field| &field.origin),
                Some(crate::snapshot::FieldOrigin::Besluit)
            ),
            "en een vast veld van het gram als zodanig"
        );
    }
}
