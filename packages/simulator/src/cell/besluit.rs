//! Het besluit-pad: een cel voert een wet uit en legt de uitkomst vast.
//!
//! De lus van chronolexografie is informeren → concluderen → **vastleggen**, en
//! dit is het derde deel. Een besluit-definitie is data, net als een
//! lexostatus-definitie: ze noemt een eigen regeling, de uitkomst die het
//! besluit *is*, waar de inputs vandaan komen en hoe het zaakkenmerk gevormd
//! wordt. Wat eruit komt is een [`Decretogram`]: het RFC-013 Execution Receipt
//! plus het moment en het zaakkenmerk, vastgelegd als gewoon executogram in de
//! eigen kroniek van de cel.
//!
//! Twee dingen die dit pad met opzet *niet* doet:
//!
//! - **Het bewaart niets naast het decretogram.** Een waarde die het besluit
//!   ophaalde, gaat mee ín het decretogram (met haar herkomst) en niet als los
//!   feit in een kroniek. Anders zou een volgend besluit of een volgende
//!   reductie op andermans feiten leunen zonder opnieuw te kijken — de
//!   schaduwboekhouding die RFC-022 uitsluit.
//! - **Het rekent niet in een reductie.** [`crate::Cell::reduce`] leest het
//!   decretogram terug als kroniekfeit; ligt er geen, dan is het antwoord
//!   "niets vastgesteld". Nooit een herberekening onder een inmiddels andere
//!   wetsversie.
//!
//! Een besluit kan ook **verplichtingen** voortbrengen: een bedrag dat op
//! vervaldata nagekomen moet worden (RFC-022 §1.2). Het schema daarvan wordt
//! hier uitgerekend — bij het besluit, op de uitkomst waarop besloten is — en
//! gaat mee in het gram. Het *nakomen* gebeurt elders: de klok laat een termijn
//! vervallen en dan legt de cel die de verplichting draagt een betaling vast.
//! Zie [`ObligationDue`].

use crate::cell::chronicle::{GebeurtenisSchema, SchemaVeld};
use crate::cell::config::{
    binding_name, check_documented_params, documents, engine_parameters, listing,
    parameter_listing, published_outputs, CellSurface, DocumentedParameter,
};
use crate::cell::extensions::{afwijzing_wanneer, ChronolexBlock};
use crate::cell::{ChronicleEvent, Intake, ParameterType, PartyBindings};
use crate::error::{Result, SimulatorError, Subject};
use crate::values::amount;
use chrono::{Months, NaiveDate};
use regelrecht_engine::{Article, ExecutionReceipt, Value};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Het rechtskarakter dat een uitkomst tot een besluit maakt.
///
/// Een decretogram **is** een engine-uitkomst met `legal_character: BESCHIKKING`
/// (RFC-022 §1.2, ontwerpprincipe 3). Een toets, een waardebepaling of een
/// feitelijke handeling is er geen: wie die als besluit zou vastleggen, legt een
/// gram in de stroom met beschikkingen dat geen beschikking is, en dan zegt een
/// reductie erover iets anders dan de wet. Het optuigen weigert daarom elke
/// besluit-definitie waarvan de aansturende uitkomst onder een geladen versie
/// iets anders draagt dan deze waarde.
pub const BESCHIKKING: &str = "BESCHIKKING";

/// Het besluittype van een besluit dat op een voorwaarde afketst.
///
/// Awb 1:3 lid 2: een beschikking omvat ook de **afwijzing** van de aanvraag.
/// Een besluit dat afketst is dus geen mislukte uitvoering en geen bedrag nul,
/// maar een decretogram als elk ander — met dit type, met de uitkomsten die de
/// regeling wél leverde, en zonder verplichtingen. Uit het vocabulaire van
/// `produces.decision_type` in het schema, zodat het gram hetzelfde woord draagt
/// als de regeling die de afwijzing zelf aanwijst.
pub const AFWIJZING: &str = "AFWIJZING";

/// De kroniekstroom waarin een cel haar eigen decretogrammen legt.
///
/// Eén vaste naam per cel, automatisch aanwezig zodra de cel besluit-definities
/// heeft, en voorbehouden: een configuratie die zelf een stroom met deze naam
/// declareert wordt geweigerd. Dat de naam vastligt, is wat een reductie erover
/// mogelijk maakt zonder dat elke casus hem opnieuw verzint.
pub const BESCHIKKINGEN: &str = "beschikkingen";

/// De kroniekstroom waarin betalingen op verplichtingen landen.
///
/// Platformvocabulaire, net als [`Intake`]: een cel die een verplichting draagt
/// of er een oplegt, houdt een stroom met deze naam. Anders dan
/// [`BESCHIKKINGEN`] wordt ze niet automatisch aangemaakt — een betaling is een
/// gewoon feit dat een cel overkomt, en een wereld mag er een startstand in
/// hebben staan. Wat wél vastligt is de naam en de sleutel ([`ZAAKKENMERK`]):
/// daarop reduceert "betaald tot nu toe", en die hoeft geen casus opnieuw te
/// verzinnen.
pub const BETALINGEN: &str = "betalingen";

/// De veldnamen die elk decretogram draagt, naast de uitkomsten van het besluit.
///
/// Ze staan hier bij elkaar omdat twee plekken ze allebei moeten kennen: het
/// vastleggen (welke velden krijgt het gram) en het optuigen (waarover mag een
/// lexostatus over deze stroom iets beloven). Zouden die uit elkaar lopen, dan
/// zou een reductie over een bestaand veld als typfout geweigerd worden.
pub const ZAAKKENMERK: &str = "zaakkenmerk";
/// Veld met de naam van de besluit-definitie die het gram voortbracht.
pub const BESLUIT: &str = "besluit";
/// Veld met de `$id` van de uitgevoerde regeling.
pub const REGULATION: &str = "regulation";
/// Veld met de `valid_from` van de regelingversie die gold op `op_moment`.
pub const REGULATION_VALID_FROM: &str = "regulation_valid_from";
/// Veld met álle regelingen die deze uitvoering uitvoerde, met hun versie.
///
/// Naast [`REGULATION`] en [`REGULATION_VALID_FROM`], die alleen de regeling
/// noemen waarop het besluit *gaat*. Wat zij aanriep staat nergens anders in het
/// gram: het receipt draagt met `scope.loaded_regulations` wat er geladen was en
/// met `results.output_provenance` waar elke uitkomst vandaan kwam, maar niet
/// welke regeling welke *input* leverde. Zonder dit veld zou het journaal een
/// verhaal vertellen dat in geen enkele kroniek terug te vinden is.
pub const EXECUTED_REGULATIONS: &str = "executed_regulations";
/// Veld met het bevoegd gezag dat de regeling noemt (RFC-002).
pub const COMPETENT_AUTHORITY: &str = "competent_authority";
/// Veld met de identiteit van de cel die besloot.
///
/// Naast [`COMPETENT_AUTHORITY`] en niet in plaats daarvan: het eerste is wat de
/// wet aanwijst, dit is wie er feitelijk besloot. Ze zijn hier gelijk — anders
/// was het besluit geweigerd — en dat ze los in het gram staan, is wat dat
/// leesbaar houdt zodra er ooit ondertekend wordt.
pub const BESLOTEN_DOOR: &str = "besloten_door";
/// Veld met het rechtskarakter dat de regeling aan deze uitkomst geeft.
pub const LEGAL_CHARACTER: &str = "legal_character";
/// Veld met het besluittype van dit gram.
///
/// Naast [`LEGAL_CHARACTER`] en niet in plaats daarvan: het rechtskarakter zegt
/// dát dit een beschikking is, dit zegt wélke. Een verlening en een weigering
/// zijn allebei een beschikking (Awb 1:3 lid 2), en zonder dit veld zou een
/// lezer het verschil uit een bedrag moeten afleiden — precies de verwarring
/// waar "een weigering is geen bedrag" over gaat.
///
/// `null` als de regeling er niets over zegt: dat is een gat in die regeling en
/// geen uitnodiging om het hier in te vullen. Bij een afwijzing staat er altijd
/// [`AFWIJZING`], want dan geldt de voorwaarde die het artikel zelf declareerde
/// boven het type dat het voor de gewone afloop noemt.
pub const DECISION_TYPE: &str = "decision_type";
/// Veld met de afwijzingsvoorwaarden die vervuld waren.
///
/// Leeg bij elk besluit dat niet afketste, en dan is dat ook wat er staat: een
/// lege lijst. Net als [`OBLIGATIONS`] altijd aanwezig — een veld dat er soms
/// niet is, laat een reductie erover afwisselend wel en niet iets vinden.
pub const AFWIJZINGSGROND: &str = "afwijzingsgrond";
/// Veld met de stage van de procedure waarin dit gram ontstond (RFC-008).
///
/// Elke stage van een procedure levert een **eigen elementair gram** op hetzelfde
/// zaakkenmerk (RFC-022 §1.2), en dit veld zegt welke. Een besluit draagt
/// [`STAGE_BESLUIT`], de bekendmaking ervan [`STAGE_BEKENDMAKING`]. Dat het er
/// als gewoon veld staat, is wat een reductie erop laat filteren (`where`): de
/// vraag "wat is er besloten" en de vraag "is het bekendgemaakt" gaan over
/// dezelfde zaak en over twee verschillende grammen.
pub const STAGE: &str = "stage";
/// De stage waarin het besluit zelf genomen wordt.
///
/// Ook de stage van elk gram dat vóór deze stap ontstond: een decretogram *is*
/// het besluit. Hetzelfde woord als waarmee de engine haar hooks indexeert
/// (RFC-008), en met opzet niet een eigen vertaling ernaast.
pub const STAGE_BESLUIT: &str = "BESLUIT";
/// De stage waarin het besluit bekendgemaakt wordt (Awb 3:40 jo. 3:41).
pub const STAGE_BEKENDMAKING: &str = "BEKENDMAKING";
/// Veld met de identiteit van de cel die bekendmaakte.
pub const BEKENDGEMAAKT_DOOR: &str = "bekendgemaakt_door";
/// Veld met de herkomst van elke uitkomst die de hooks op deze stage leverden.
///
/// Per uitkomst het lexogram dat haar voortbracht: de regeling, de versie die
/// gold en het artikel. Zonder die herkomst staat er wel een uiterste
/// betaaldatum in het gram, maar niet uit welke wet ze komt — en dan is het
/// verschil tussen "de Awb zegt het" en "de uitvoerder heeft het bedacht" niet
/// meer te zien.
pub const HOOKS: &str = "hooks";
/// Veld met de herkomst van elke uitkomst die de **eigen regeling** van het
/// besluit bij deze stage oplevert.
///
/// De tegenhanger van [`HOOKS`] voor wat het uitvoerende artikel zelf aan een
/// latere stage hangt (`stage_uitkomsten` in het `chronolex`-blok): per uitkomst
/// de regeling, de versie en het artikel. Apart van de hooks, want het is iets
/// anders — een hook is de algemene wet die op elke beschikking van een soort
/// vuurt, dit is de regeling van dít besluit.
pub const STAGE_UITKOMSTEN: &str = "stage_uitkomsten";
/// Veld met de hooks die op dit gram vuurden maar niet konden draaien.
///
/// Een hook vuurt op rechtskarakter, besluittype en stage, en niet op wat het
/// besluit waarop hij vuurt aan feiten draagt. Ontbreekt een input die hij
/// nodig heeft, dan valt het besluit daar niet op om: het gram noemt per hook
/// het artikel en de input die er niet was. Altijd aanwezig, en leeg als elke
/// hook draaide — net als [`AFWIJZINGSGROND`].
pub const HOOK_NIET_UITGEVOERD: &str = "hook_niet_uitgevoerd";
/// Veld met de termijnen die op de bekendmaking wachten.
///
/// Wat het besluit oplegde maar nog niet kon inroosteren: het bedrag, de
/// partijen en de grondslag staan er al, alleen de vervaldatum niet — die volgt
/// uit de bekendmaking. Leeg bij elk besluit waarvan de termijnen meteen
/// vervielen, zoals [`OBLIGATIONS`] leeg is bij een besluit dat niets oplegt.
pub const WACHT_OP_BEKENDMAKING: &str = "wacht_op_bekendmaking";
/// Veld waarmee een bekendmaking zegt dat er niets meer in te roosteren viel.
///
/// Een beschikking die vervangen wordt vóórdat ze bekendgemaakt is, heeft bij
/// haar bekendmaking niets meer te beloven: de belofte zou pas op dat moment
/// gaan werken (Awb 3:40), en over dezelfde zaak ligt inmiddels een besluit dat
/// in de plaats van dit besluit kwam (zie [`Vervanging`]). Het gram draagt dan
/// welk besluit dat was, op welk moment en op welke grondslag, en
/// [`OBLIGATIONS`] blijft leeg. `null` bij elke bekendmaking waarbij dat niet
/// speelde — zoals [`WACHT_OP_BEKENDMAKING`] leeg is bij een besluit dat niets
/// liet wachten.
pub const TERMIJNEN_VERVALLEN_DOOR: &str = "termijnen_vervallen_door";
/// Veld met de verplichtingen waarvan het bedrag op precies nul uitkwam.
///
/// Een verplichting van nul is geen termijn van nul. Er valt niets te betalen,
/// dus er hoort geen vervaldag en geen executogram bij — een betaling van nul
/// euro is een vastlegging van iets dat niet gebeurde. Weglaten kan evenmin: het
/// artikel legde haar wél op, en een gram dat erover zwijgt is niet te
/// onderscheiden van een besluit waarop de verplichting niet van toepassing was.
/// Ze staat hier dus met `bedrag: 0`, haar partijen en haar grondslag, en zonder
/// termijnen. Leeg bij elk besluit waarvan geen verplichting op nul uitviel.
pub const NIETS_TE_BETALEN: &str = "niets_te_betalen";
/// Veld met de inputs van het besluit, elk met hun herkomst.
pub const INPUTS: &str = "inputs";
/// Veld met het uitgerekende betalingsschema van dit besluit.
pub const OBLIGATIONS: &str = "obligations";
/// Veld van een ingehaalde termijn met de dag waarop ze volgens het schema had
/// moeten vervallen.
///
/// Alleen bij een termijn die **ingehaald** wordt: haar dag lag vóór het moment
/// waarop er betaald kón worden — het besluit, of bij `vanaf: bekendmaking` de
/// bekendmaking — en ze vervalt daarom op die dag (zie [`inhalen`]). Een termijn
/// die op haar eigen dag vervalt, draagt het veld niet: dan zou elk gram een
/// tweede datum dragen die niets zegt.
pub const OORSPRONKELIJKE_VERVALDATUM: &str = "oorspronkelijke_vervaldatum";
/// Veld met de eigen kronieken waarop de uitvoering leunde, met hun stand.
pub const CHRONICLE_SOURCES: &str = "chronicle_sources";
/// Veld met het volledige RFC-013 Execution Receipt.
pub const RECEIPT: &str = "receipt";

/// De vaste velden van een decretogram, in de volgorde waarin ze hierboven staan.
///
/// Van de stage BESLUIT: dit is wat een gram draagt waarin een besluit staat.
/// Wat de **bekendmaking** ervan draagt, staat in [`BEKENDMAKING_FIELDS`] — een
/// eigen gram met een eigen omslag, en niet een tweede lijst voor hetzelfde gram.
const FIXED_FIELDS: [&str; 18] = [
    ZAAKKENMERK,
    BESLUIT,
    STAGE,
    WACHT_OP_BEKENDMAKING,
    NIETS_TE_BETALEN,
    REGULATION,
    REGULATION_VALID_FROM,
    EXECUTED_REGULATIONS,
    COMPETENT_AUTHORITY,
    BESLOTEN_DOOR,
    LEGAL_CHARACTER,
    DECISION_TYPE,
    AFWIJZINGSGROND,
    HOOK_NIET_UITGEVOERD,
    INPUTS,
    OBLIGATIONS,
    CHRONICLE_SOURCES,
    RECEIPT,
];

/// De vaste velden van het gram van een **bekendmaking**, in dezelfde volgorde.
///
/// Apart van [`FIXED_FIELDS`] omdat het een ander gram is: een bekendmaking legt
/// niets vast over de uitkomsten van het besluit — die staan in het besluit — en
/// draagt in plaats daarvan wie haar deed, wat er bij haar werd ingevuld (de
/// `requires` van de stage, als inputs), waar het besluit ligt waar ze bij hoort,
/// en waar elk veld vandaan komt dat de wet er bij deze stage aan hangt. De dag
/// van de bekendmaking is het `op_moment` van het gram zelf.
const BEKENDMAKING_FIELDS: [&str; 9] = [
    STAGE,
    BEKENDGEMAAKT_DOOR,
    INPUTS,
    HOOKS,
    STAGE_UITKOMSTEN,
    HOOK_NIET_UITGEVOERD,
    BESLUIT_OP_MOMENT,
    BESLUIT_GRAM,
    TERMIJNEN_VERVALLEN_DOOR,
];

/// De vaste velden van een decretogram, in de volgorde waarin ze hierboven staan.
///
/// Voor het beeld van de wereld: alleen wie deze namen kent, kan van elk veld van
/// een decretogram zeggen of het een uitkomst van het besluit is of een vast veld
/// van het gram zelf — en dus waar de waarde vandaan komt.
pub(crate) fn fixed_fields() -> &'static [&'static str] {
    &FIXED_FIELDS
}

/// Elke vaste veldnaam die in de stroom [`BESCHIKKINGEN`] kan voorkomen, van
/// welke stage ook.
///
/// Voor de twee vragen die over de **stroom** gaan en niet over één gram: wat mag
/// een reductie erover publiceren, en welke naam is bezet voor een uitkomst van
/// een besluit. Zou die tweede alleen de velden van het besluit kennen, dan kon
/// een uitkomst `stage_uitkomsten` heten en in dezelfde stroom twee dingen
/// betekenen.
pub(crate) fn beschikkingen_fields() -> BTreeSet<&'static str> {
    FIXED_FIELDS
        .iter()
        .chain(BEKENDMAKING_FIELDS.iter())
        .copied()
        .collect()
}

/// Veld met het bedrag van één betalingstermijn.
pub const BEDRAG: &str = "bedrag";
/// Veld met de grondslag waarop een verplichting berust, uit het lexogram.
pub const GRONDSLAG: &str = "grondslag";
/// Veld met de herkomst van een verplichting: het lexogram dat haar declareert.
pub const LEXOGRAM: &str = "lexogram";
/// Veld met het artikelnummer binnen die herkomst.
pub const ARTIKEL: &str = "artikel";
/// Veld met het volgnummer van een termijn binnen één verplichting.
pub const VOLGNUMMER: &str = "volgnummer";
/// Veld met de cel die het besluit nam waaruit deze betaling volgt.
pub const BESLUIT_CEL: &str = "besluit_cel";
/// Veld met het moment van dat besluit.
pub const BESLUIT_OP_MOMENT: &str = "besluit_op_moment";
/// Veld met de kroniekstroom waarin het decretogram van dat besluit ligt.
///
/// Altijd [`BESCHIKKINGEN`] — een decretogram ligt nergens anders — en toch als
/// veld, want samen met [`BESLUIT_CEL`] en [`BESLUIT_GRAM`] vormt het de
/// volledige verwijzing `<cel>|<kroniek>|<plek>` waarmee het beeld van de wereld
/// een gram aanwijst. Twee van de drie opschrijven en de derde laten raden, zou
/// de verwijzing afhankelijk maken van een afspraak buiten het gram.
pub const BESLUIT_KRONIEK: &str = "besluit_kroniek";
/// Veld met de plek van dat decretogram in die stroom, geteld vanaf nul.
///
/// Hiermee komt een lezer van de betaling bij het besluit zelf — en dus bij het
/// receipt en de uitvoeringstrace daarin. Zonder deze plek staat er wel *welk*
/// besluit het was, maar moet een lezer de kroniek van de besluitende cel
/// aflopen om het te vinden, en dat is precies het werk dat een verwijzing hoort
/// weg te nemen (RFC-013: het besluit moet na te lopen zijn).
pub const BESLUIT_GRAM: &str = "besluit_gram";

/// Veld met de soort verplichting waaruit een termijn volgt.
pub const SOORT: &str = "soort";
/// Veld met de partij die de verplichting moet nakomen.
///
/// Een **naam uit het recht** en geen cel: de wet wijst een bevoegd gezag aan en
/// een besluit gaat over een aanvrager, en geen van beide weet hoe iemand zijn
/// uitvoering heeft ingericht. Welke cel er onder die naam betaalt, staat in het
/// wereldbestand — zie [`BETALER`].
pub const SCHULDENAAR: &str = "schuldenaar";
/// Veld met de partij aan wie nagekomen moet worden.
pub const SCHULDEISER: &str = "schuldeiser";
/// Veld met de cel die de termijn namens de schuldenaar nakomt.
///
/// `null` als deze wereld geen cel voor de schuldenaar kent. Dat is geen fout en
/// geen gat in het gram: de verplichting staat er, de termijn wordt ingeroosterd,
/// en er is alleen niemand in deze wereld die haar nakomt. Zie
/// [`ObligationDue::betaler`].
pub const BETALER: &str = "betaler";

/// Het ingebouwde gebeurtenisschema van de stroom [`BETALINGEN`].
///
/// Het platform declareert wat een nagekomen verplichting vastlegt, en het
/// declareert het als **schema** en niet als lijst veldnamen in Rust: dat een
/// betaling een zaakkenmerk, een bedrag, een termijnnummer, het besluit waaruit
/// ze volgt en twee partijen draagt, is het typeschema van een executogram —
/// generiek, compile-time, en dus data (RFC-022 §1.3).
///
/// Wat een wereldbestand ermee moet: een cel die deze verplichting nakomt, en de
/// cel die haar oplegde, houden een stroom [`BETALINGEN`] waarvan het schema
/// hieraan voldoet (zie [`crate::SimulatorError::ObligationStream`]). Het
/// wereldbestand schrijft het dus zelf op, en het platform toetst het — in
/// plaats van dat alleen Rust weet wat er in die stroom hoort.
///
/// Een **ondergrens**: het gram draagt meer dan dit (de verwijzing naar het gram
/// van het besluit, bijvoorbeeld), en een wereld die daarover wil reduceren
/// declareert die velden erbij. Wat hier staat is wat elke betaling hoe dan ook
/// draagt en waar dus over te rekenen valt.
pub(crate) fn nakoming_schema(soort: ObligationKind) -> Vec<GebeurtenisSchema> {
    let velden = || {
        [
            (ZAAKKENMERK, ParameterType::String),
            (BEDRAG, ParameterType::Amount),
            (VOLGNUMMER, ParameterType::Number),
            (BESLUIT, ParameterType::String),
            (SCHULDENAAR, ParameterType::String),
            (SCHULDEISER, ParameterType::String),
        ]
        .into_iter()
        .map(|(name, value_type)| SchemaVeld {
            name: name.to_string(),
            value_type,
        })
        .collect::<Vec<_>>()
    };
    vec![
        GebeurtenisSchema {
            name: soort.gedaan(),
            intake: Intake::Betaling,
            // Leeg, en niet de grondslag uit het lexogram: welke wet, welk
            // artikel en welke termijn het is, verschilt per gram, en dat schrijft
            // [`ObligationDue::grondslag`] er dan ook per gram bij. Een schema dat
            // hier iets algemeens zou zeggen, zou dat overschrijven noch aanvullen
            // — zie [`GebeurtenisSchema::grondslag`].
            grondslag: String::new(),
            fields: velden(),
        },
        GebeurtenisSchema {
            name: soort.gemeld(),
            intake: Intake::Levering,
            grondslag: String::new(),
            fields: velden(),
        },
    ]
}

/// Eén besluit dat een cel kan nemen.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BesluitDefinition {
    /// De naam waarmee dit besluit aangeroepen wordt.
    pub name: String,
    /// Vrije toelichting; verschijnt niet in het decretogram, wel in de config.
    #[serde(default)]
    pub doc: Option<String>,
    /// De regeling die uitgevoerd wordt, bij `$id`. Moet in `laws` van dezelfde
    /// cel staan: een cel besluit op haar eigen recht.
    pub regulation: String,
    /// De uitkomst die dit besluit *is* en die de uitvoering aanstuurt.
    pub output: String,
    /// Uitkomsten die naast [`Self::output`] in het decretogram meegaan.
    ///
    /// Wat samen ontstaat, wordt samen vastgelegd (RFC-022 §1.2): één besluit is
    /// één elementair gram met al zijn uitkomsten erin, niet één gram per
    /// uitkomst.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Het zaakkenmerk, als sjabloon met `{parameter}`-verwijzingen.
    ///
    /// Dit is waaronder de zaak terug te vinden is: de kroniek van
    /// decretogrammen over één zaak deelt één zaakkenmerk (RFC-022 §1.2).
    pub zaakkenmerk: String,
    /// De gedocumenteerde parameters van dit besluit.
    #[serde(default)]
    pub params: Vec<DocumentedParameter>,
    /// Wat het besluit aan de engine aanlevert, op inputnaam.
    ///
    /// De naam is die van een parameter of input van de regeling; de waarde
    /// zegt waar hij vandaan komt: uit een eigen kroniek van de cel, uit de
    /// parameters van het besluit, of **geaccepteerd** van een andere cel
    /// (`accept_from`).
    #[serde(default)]
    pub inputs: BTreeMap<String, BesluitInput>,
    /// Hier **niet** toegestaan: wanneer een besluit afwijst, staat in het
    /// lexogram.
    ///
    /// Het veld staat in deze struct om geweigerd te kunnen worden met een
    /// melding die de weg wijst ([`SimulatorError::AfwijzingWanneerInWereldbestand`]).
    /// Zonder het veld zou `deny_unknown_fields` er "onbekend veld" van maken,
    /// en dat vertelt niet waar de regel dan wél hoort: op het uitvoerende
    /// artikel, onder [`AFWIJZING_WANNEER`](crate::cell::extensions::AFWIJZING_WANNEER)
    /// in de namespace [`CHRONOLEX`](crate::cell::extensions::CHRONOLEX) van
    /// `produces.extensions`.
    ///
    /// Vrij van vorm en niet getypeerd: elke poging hoort dezelfde melding te
    /// krijgen, ook een met een typfout erin.
    #[serde(default)]
    pub afwijzing_wanneer: Option<Value>,
    /// **Vervallen.** Verplichtingen staan in het lexogram, niet hier.
    ///
    /// Het veld blijft bekend om precies één reden: een wereldbestand dat het
    /// nog draagt hoort een melding te krijgen die zegt waar de verplichtingen
    /// wél horen, en niet een kale "unknown field" van serde. Zie
    /// [`SimulatorError::ObligationsInWorldFile`] en
    /// [`ObligationDefinition`] voor de plek die ervoor in de plaats kwam.
    #[serde(default)]
    pub obligations: ObsoleteField,
}

/// Een veld dat vervallen is: staat het er nog, dan hoort dat een melding te
/// geven en geen stilte.
///
/// Met opzet **geen** `Option`: serde laat een `Option` op een `null` als "niet
/// aanwezig" eindigen, en dan glipt een half omgezet wereldbestand dat de lijst
/// leeghaalde (`obligations:` met niets erachter) langs precies de melding
/// waarvoor het veld nog bestaat. De sleutel is de vraag, niet wat erachter
/// staat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ObsoleteField {
    /// De sleutel staat er niet, en zo hoort het.
    #[default]
    Absent,
    /// De sleutel staat er nog, wat er ook achter staat — ook `null`.
    Present,
}

impl<'de> Deserialize<'de> for ObsoleteField {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        serde::de::IgnoredAny::deserialize(deserializer)?;
        Ok(Self::Present)
    }
}

/// De enige soort verplichting die een lexogram mag declareren.
///
/// Platformvocabulaire, zoals [`Schedule`]: een soort erbij is een variant
/// erbij, en een typfout hoort niet stil als betaling te eindigen.
pub const BETALING: &str = "betaling";

/// De soort die een **omgekeerde** verplichting draagt (Awb 4:57).
///
/// Niet te declareren, en dat is het punt: ze ontstaat doordat het bedrag
/// negatief uitvalt en het lexogram voor dat geval
/// [`RichtingBijNegatief::Omkeren`] declareert. Wie haar wél mocht opschrijven,
/// kon iemand laten terugbetalen zonder dat er ooit iets te veel betaald is.
pub const TERUGVORDERING: &str = "terugvordering";

/// Het achtervoegsel van het gram waarin de nakomende cel haar nakoming legt.
///
/// Twee gebeurtenissen per soort verplichting en niet één, want het zijn twee
/// verschillende feiten in twee verschillende kronieken: de betaler betaalde, en
/// de besluitende cel kreeg gemeld dát er betaald is. Een naam die dat verschil
/// niet maakt, laat een som over één stroom over beide kanten lopen.
const GEDAAN: &str = "_gedaan";

/// Het achtervoegsel van het gram waarin de besluitende cel de melding legt.
const GEMELD: &str = "_gemeld";

/// De verwijzing waarmee een verplichting het bevoegd gezag als partij aanwijst.
///
/// Dezelfde resolutie als bij het besluit zelf: het `competent_authority` van het
/// artikel, en anders dat van het document (RFC-002). Eén ingebouwde naam en geen
/// vrije `#`-verwijzing — wie hier een willekeurige uitkomst mocht noemen, kon de
/// schuldenaar van een verplichting laten afhangen van wat de engine uitrekende.
pub const BEVOEGD_GEZAG_REFERENCE: &str = "#bevoegd_gezag";

/// Welke kant een verplichting op staat, zoals het gram haar draagt.
///
/// Geen open vocabulaire: de twee soorten zijn elkaars spiegelbeeld, en welke van
/// de twee het is, volgt uit het teken van het bedrag en uit wat het lexogram
/// daarover declareert — nooit uit een woord dat iemand erbij typt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationKind {
    /// Het gewone geval: de schuldenaar betaalt wat het besluit toekent.
    Betaling,
    /// De omgekeerde: wat te veel betaald is, moet terug (Awb 4:57).
    Terugvordering,
}

impl ObligationKind {
    /// De naam waaronder deze soort in een gram staat.
    pub fn name(self) -> &'static str {
        match self {
            Self::Betaling => BETALING,
            Self::Terugvordering => TERUGVORDERING,
        }
    }

    /// De soort bij haar naam, zoals ze in een gram staat.
    ///
    /// De tegenhanger van [`Self::name`], en op dezelfde twee namen: een gram dat
    /// een derde naam draagt is niet te lezen, en dat hoort te blijken in plaats
    /// van als betaling door te gaan.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            BETALING => Some(Self::Betaling),
            TERUGVORDERING => Some(Self::Terugvordering),
            _ => None,
        }
    }

    /// De naam van het gram waarmee de nakomende cel vastlegt dát ze nakwam.
    pub fn gedaan(self) -> String {
        format!("{}{GEDAAN}", self.name())
    }

    /// De naam van het gram waarmee de besluitende cel vastlegt dat het haar
    /// gemeld is.
    pub fn gemeld(self) -> String {
        format!("{}{GEMELD}", self.name())
    }

    /// Dezelfde verplichting, de andere kant op.
    fn reversed(self) -> Self {
        match self {
            Self::Betaling => Self::Terugvordering,
            Self::Terugvordering => Self::Betaling,
        }
    }
}

/// Wat een verplichting doet als het bedrag negatief uitvalt.
///
/// Eén variant, en met opzet geen `bool`: een wereldbestand dat `omkeeren` typt
/// hoort een melding te krijgen die de vorm noemt, en niet stil bij de standaard
/// te eindigen. De standaard is er geen: zonder declaratie is een negatief bedrag
/// een fout bij het besluit (zie [`SimulatorError::NegativeObligationAmount`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RichtingBijNegatief {
    /// Schuldenaar en schuldeiser wisselen, het bedrag wordt positief, en wat
    /// eruit komt is een [`TERUGVORDERING`].
    Omkeren,
}

/// Dat een beschikking in de plaats komt van wat er over dezelfde zaak nog
/// openstond.
///
/// Een verplichting is niet in te trekken: haar schema staat in een gram, en een
/// gram verandert niet. Een tweede beschikking over dezelfde zaak laat de
/// termijnen van de eerste dus gewoon vervallen — tenzij de wet zegt dat deze
/// beschikking de vorige **vervangt**. Dat is wat dit declareert, en daarom staat
/// het in het lexogram en niet in een wereldbestand: of een vaststelling het
/// voorschot vervangt, is recht (Awir art. 19 jo. art. 24, tweede lid) en geen
/// keuze van de uitvoerder.
///
/// Wat er vervalt, is wat op het moment van dit besluit nog niet verstreken wás.
/// Termijnen die al verstreken zijn, blijven staan: wat betaald is, is betaald, en
/// wat ermee moet gebeuren is een verrekening en geen terugdraaiing.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vervanging {
    /// Waarop het vervangen berust, in vrije tekst.
    ///
    /// Verplicht, en om dezelfde reden als bij een verplichting: een termijn die
    /// vervalt zonder grondslag is een belofte die zonder wet verdwijnt. Ze komt
    /// in het journaal te staan bij elke termijn die erdoor vervalt.
    pub grondslag: String,
}

/// Het besluit dat in de plaats kwam van een beschikking die nog bekendgemaakt
/// moest worden.
///
/// De andere kant van [`Vervanging`], en een ander moment: bij een besluit
/// vervalt wat er ingeroosterd stond, hier gaat wat er nog wachtte niet meer
/// lopen. Een verplichting met `vanaf: bekendmaking` staat niet in de wachtrij
/// maar in het gram van haar eigen besluit ([`WACHT_OP_BEKENDMAKING`]), dus de
/// vervanging komt er niet bij — ze gaat pas werken bij de bekendmaking (Awb
/// 3:40), en dáár valt te zien dat er niets meer te beloven is.
///
/// Wat hier staat, komt in het gram van de bekendmaking te staan
/// ([`TERMIJNEN_VERVALLEN_DOOR`]) en in het journaal: welk besluit ervoor in de
/// plaats kwam, wanneer, en op welke grondslag het dat mocht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermijnenVervallen {
    /// Het besluit dat in de plaats kwam.
    pub besluit: String,
    /// Het moment waarop dat besluit genomen is.
    pub op_moment: NaiveDate,
    /// De grondslag waarop dat besluit de openstaande termijnen vervangt, uit
    /// het lexogram van het artikel dat het voortbracht.
    pub grondslag: String,
}

impl TermijnenVervallen {
    /// Als vastlegbare waarde, voor in het gram van de bekendmaking.
    pub(crate) fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            (BESLUIT.to_string(), Value::String(self.besluit.clone())),
            (
                "op_moment".to_string(),
                Value::String(self.op_moment.to_string()),
            ),
            (GRONDSLAG.to_string(), Value::String(self.grondslag.clone())),
        ]))
    }

    /// Leesbare regel voor het journaal: waarom er niets gaat lopen.
    pub(crate) fn describe(&self) -> String {
        format!(
            "besluit '{}' van {} kwam ervoor in de plaats, op grondslag '{}'",
            self.besluit, self.op_moment, self.grondslag
        )
    }
}

/// Eén verplichting die een besluit oplegt, zoals het **lexogram** haar declareert.
///
/// Ze staat in het artikel dat het besluit uitvoert (`produces.extensions.
/// chronolex.verplichtingen`) en niet in het wereldbestand: dát er betaald moet
/// worden en in welk ritme, schrijft de wet voor. Twee uitvoerders van dezelfde
/// regeling krijgen daarmee hetzelfde verplichtingenschema, ook als ze verder
/// niets van elkaar weten.
///
/// Wat er niet in staat is even belangrijk: geen kroniekstroom (die heet
/// [`BETALINGEN`], overal), geen bedrag met de hand (dat zou naast de
/// wetsuitkomst gaan leven), geen vervaldata per stuk (die volgen uit het ritme)
/// en **geen betalende cel** — wélk systeem namens de schuldenaar betaalt, is
/// uitvoering en staat in het wereldbestand (`komt_na`). Wat er wél in staat is
/// de rechtsverhouding: wie schuldenaar is en wie schuldeiser, want dat is wat de
/// wet aanwijst.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationDefinition {
    /// Wat voor verplichting dit is; vandaag alleen [`BETALING`].
    pub soort: String,
    /// Het bedrag, als `$uitkomst` van het besluit dat dit artikel aanstuurt.
    ///
    /// Geen letterlijk bedrag en geen parameter: wat betaald moet worden, komt
    /// uit de wet zelf. Een bedrag dat er los naast staat zou naast de uitkomst
    /// gaan leven, en dan zegt het gram twee dingen over hetzelfde geld.
    pub bedrag: String,
    /// Het ritme: `ineens`, `kwartaal` of `maand`, of `$instelling` — een
    /// verwijzing naar `settings` in het wereldbestand.
    ///
    /// Dat een ritme een instelling mag zijn, is geen gemak: een betalingsritme
    /// is doorgaans beleid en geen wet, en beleid hoort niet in een regeling
    /// vast te staan alsof de wet het voorschrijft.
    pub ritme: String,
    /// Wie moet nakomen: [`BEVOEGD_GEZAG_REFERENCE`] of `$parameter`.
    ///
    /// Weggelaten is het bevoegd gezag — het gewone geval, want een beschikking
    /// die een bedrag toekent laat het bestuursorgaan betalen. Dat die standaard
    /// bestaat, betekent niet dat het gram hem mag verzwijgen: hij wordt bij het
    /// optuigen expliciet gemaakt en staat in elke termijn (zie
    /// [`ObligationDue::schuldenaar`]).
    #[serde(default)]
    pub schuldenaar: Option<String>,
    /// Aan wie nagekomen moet worden; dezelfde twee vormen als [`Self::schuldenaar`].
    ///
    /// Weggelaten is de parameter waarmee het zaakkenmerk de zaak identificeert:
    /// een beschikking gaat over iemand, en dát is de partij die het geld krijgt.
    /// Wijst het zaakkenmerk niet precies één parameter aan, dan valt er niets te
    /// raden en moet de declaratie het zeggen.
    #[serde(default)]
    pub schuldeiser: Option<String>,
    /// Wat er gebeurt als het bedrag negatief uitvalt; weggelaten is: dat is een
    /// fout.
    ///
    /// Een negatieve betaling bestaat niet. Wat een vaststelling lager dan het
    /// voorschot oplevert, is juridisch een **terugvordering** (Awb 4:57): een
    /// verplichting de andere kant op, met de partij als schuldenaar. Dat is een
    /// andere rechtsverhouding en geen minteken, dus de wet moet hem declareren —
    /// zwijgt ze, dan valt het besluit om en wordt er niets vastgelegd.
    #[serde(default)]
    pub richting_bij_negatief: Option<RichtingBijNegatief>,
    /// Vanaf wanneer de termijnen lopen, als sjabloon met `{parameter}`.
    ///
    /// Weggelaten betekent: het moment van het besluit. Wat er staat moet een
    /// datum opleveren (`JJJJ-MM-DD`). Ligt die vóór het besluit, dan worden de
    /// termijnen die al vervallen hadden moeten zijn op de dag van het besluit
    /// ingehaald — betalen kan niet vóór het besluit waaruit het volgt. De
    /// verwijzingen zijn die van het besluit dat het artikel uitvoert, en dus
    /// van de cel die het neemt.
    #[serde(default)]
    pub vanaf: Option<String>,
    /// Welke uitkomst van de bekendmaking de eerste vervaldag levert.
    ///
    /// Hoort bij `vanaf: bekendmaking` en alleen daar: dan staat de dag bij het
    /// besluit nog niet vast, en de wet rekent hem bij de bekendmaking uit — in
    /// een hook (de algemene wet) of in een `stage_uitkomsten` van de eigen
    /// regeling. Welke uitkomst dat is, noemt de verplichting zelf, onder de
    /// naam die de wet eraan geeft; het platform kent er geen vaste naam voor.
    #[serde(default)]
    pub vervaldatum: Option<String>,
    /// Waarop deze verplichting berust, in vrije tekst.
    ///
    /// Verplicht: een verplichting zonder grondslag is een bedrag zonder wet.
    /// Ze reist mee tot in elke termijn van het decretogram, zodat wie een
    /// betaling terugleest niet alleen ziet dát er betaald moest worden maar ook
    /// waarom.
    pub grondslag: String,
}

/// Naar wie een verplichting wijst: het recht noemt een **partij**, geen cel.
///
/// Twee vormen en meer niet. Het bevoegd gezag dat de regeling aanwijst
/// ([`BEVOEGD_GEZAG_REFERENCE`]), of een parameter van het besluit (`$naam`)
/// waarvan de waarde de partij benoemt. Een kale naam mag niet: die zou één
/// organisatie in de wet vastspijkeren, en dan legt dezelfde regeling in een
/// andere wereld de verplichting bij iemand die er niets mee te maken heeft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PartyRef<'a> {
    /// Het bevoegd gezag van het artikel, en anders dat van het document.
    Authority,
    /// De parameter van het besluit die de partij benoemt.
    Param(&'a str),
}

impl<'a> PartyRef<'a> {
    /// Lees een declaratie; `None` als het geen van beide vormen is.
    fn parse(text: &'a str) -> Option<Self> {
        if text == BEVOEGD_GEZAG_REFERENCE {
            return Some(Self::Authority);
        }
        text.strip_prefix('$')
            .filter(|name| !name.is_empty())
            .map(Self::Param)
    }

    /// De naam waar deze verwijzing bij dit besluit op uitkomt.
    fn resolve(
        self,
        role: &str,
        cell: &str,
        definition: &BesluitDefinition,
        declared: &DeclaredObligations,
        params: &BTreeMap<String, Value>,
    ) -> Result<String> {
        match self {
            // Dezelfde naam als waarop `Cell::decide` de bevoegdheid toetste: één
            // resolutie, zodat een besluit niet onder het ene gezag genomen en
            // onder het andere nagekomen kan worden.
            Self::Authority => declared.authority.clone().ok_or_else(|| {
                SimulatorError::ObligationWithoutAuthority {
                    cell: cell.to_string(),
                    besluit: definition.name.clone(),
                    origin: declared.origin.describe(),
                }
            }),
            Self::Param(name) => params
                .get(name)
                .map(ToString::to_string)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| SimulatorError::ObligationParty {
                    cell: cell.to_string(),
                    besluit: definition.name.clone(),
                    origin: declared.origin.describe(),
                    role: role.to_string(),
                    reason: format!("'${name}' heeft bij dit besluit geen waarde"),
                }),
        }
    }
}

/// De rechtsverhouding van één verplichting, met het bedrag in de richting
/// waarin ze staat.
///
/// De vier samen, want ze volgen uit elkaar: draait het bedrag om, dan draaien de
/// partijen mee en wordt het een andere soort. Wie ze los zou uitrekenen, kon een
/// terugvordering opleveren die nog steeds naar het bestuursorgaan wijst.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ObligationRelation {
    /// Wat voor verplichting dit geworden is.
    soort: ObligationKind,
    /// De partij die moet nakomen.
    schuldenaar: String,
    /// De partij aan wie nagekomen moet worden.
    schuldeiser: String,
    /// Het bedrag, altijd positief.
    total: Decimal,
}

/// De verplichtingen van één artikel, met de plek waar ze vandaan komen.
///
/// Het artikel is de eenheid en niet de regeling: RFC-002 legt ook het bevoegd
/// gezag op het artikel, en het is hetzelfde artikel — dat de sturende uitkomst
/// voortbrengt — dat zegt wie mag besluiten én wat dat besluit oplegt.
#[derive(Debug, Clone)]
pub(crate) struct DeclaredObligations {
    /// Regeling, versie en artikel: waar deze verplichtingen staan.
    pub(crate) origin: ObligationOrigin,
    /// Het bevoegd gezag dat deze versie voor dit besluit aanwijst.
    ///
    /// Hier en niet elders, omdat het bij het optuigen samen met de
    /// verplichting nodig is: de betalende cel wordt aan dit gezag gebonden
    /// (`komt_na`), dus een verplichting onder een regeling die zwijgt, heeft
    /// niemand die haar nakomt.
    pub(crate) authority: Option<String>,
    /// De uitkomsten die dit artikel voortbrengt, voor de toets op `bedrag`.
    pub(crate) article_outputs: BTreeSet<String>,
    /// De uitkomsten die de stage BEKENDMAKING bij een besluit op dit artikel
    /// kan opleveren: die van de hooks op die stage en die van de eigen
    /// `stage_uitkomsten`. Voor de toets op `vervaldatum`.
    pub(crate) stage_outputs: BTreeSet<String>,
    /// De verplichtingen zelf, in de volgorde van het artikel.
    pub(crate) items: Vec<ObligationDefinition>,
    /// Of een beschikking op dit artikel in de plaats komt van wat er over
    /// dezelfde zaak nog openstond; `None` is: het artikel zegt er niets over,
    /// en dan blijft staan wat er staat.
    pub(crate) vervanging: Option<Vervanging>,
}

/// Van wie, voor wie en wanneer een besluit zijn verplichtingen inroostert.
///
/// De vier samen en niet elk apart: ze horen bij hetzelfde besluit, en wie ze
/// los doorgeeft kan de cel die besloot en de cel die betaalt verwisselen zonder
/// dat de compiler iets zegt.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ObligationScope<'a> {
    /// De cel die besloot.
    pub(crate) cell: &'a str,
    /// Welke cel er in deze wereld onder welke naam nakomt.
    ///
    /// De bindingen en niet één cel: wie er betaalt hangt aan de **schuldenaar**
    /// van elke verplichting apart, en die staat pas vast als het bedrag er is —
    /// een negatief bedrag draait de rollen om. Kent de wereld voor die naam geen
    /// cel, dan wordt de termijn wel ingeroosterd en niet nagekomen.
    pub(crate) parties: &'a PartyBindings,
    /// Het zaakkenmerk van dit besluit.
    pub(crate) zaakkenmerk: &'a str,
    /// Het moment van het besluit.
    pub(crate) op_moment: NaiveDate,
}

/// Waar een verplichting vandaan komt: het artikel dat haar declareert.
///
/// Reist mee tot in elke termijn van het gram. Zonder die herkomst zou een
/// termijn wel zeggen wat er betaald moet worden maar niet onder welk recht, en
/// dan is bij een volgende versie van de regeling niet meer na te lopen waarop
/// dit besluit stoelde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationOrigin {
    /// De `$id` van de regeling.
    pub regulation: String,
    /// De `valid_from` van de versie die gold, of `None` als ze die niet noemt.
    pub valid_from: Option<String>,
    /// Het nummer van het artikel dat de verplichting declareert.
    pub article: String,
}

impl ObligationOrigin {
    /// De herkomst als vastlegbare waarde, voor in het decretogram.
    fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            (
                REGULATION.to_string(),
                Value::String(self.regulation.clone()),
            ),
            (
                REGULATION_VALID_FROM.to_string(),
                optional_text(self.valid_from.as_deref()),
            ),
            (ARTIKEL.to_string(), Value::String(self.article.clone())),
        ]))
    }

    /// Lees een herkomst terug uit het gram waarin ze staat.
    ///
    /// De tegenhanger van [`Self::as_value`]; `None` zodra de regeling of het
    /// artikel er niet in staat. Een versie die ontbreekt is geen fout — een
    /// regeling hoeft er geen te noemen — en komt als `None` terug.
    fn from_value(value: &Value) -> Option<Self> {
        let Value::Object(fields) = value else {
            return None;
        };
        Some(Self {
            regulation: fields.get(REGULATION).and_then(Value::as_str)?.to_string(),
            valid_from: fields
                .get(REGULATION_VALID_FROM)
                .and_then(Value::as_str)
                .map(str::to_string),
            article: fields.get(ARTIKEL).and_then(Value::as_str)?.to_string(),
        })
    }

    /// Leesbare herkomst voor een grondslag of een verslag.
    pub(crate) fn describe(&self) -> String {
        match &self.valid_from {
            Some(valid_from) => format!(
                "{} artikel {}, versie {valid_from}",
                self.regulation, self.article
            ),
            None => format!("{} artikel {}", self.regulation, self.article),
        }
    }
}

/// Het ritme waarin een verplichting vervalt.
///
/// Drie soorten, en ze beschrijven alle drie **één jaar**: ineens is één
/// termijn, kwartaal vier, maand twaalf. Platformvocabulaire zoals [`Intake`]:
/// een ritme erbij is een variant erbij, en een typfout in een ritmenaam hoort
/// niet stil als "ineens" te eindigen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    /// Eén termijn, op de startdatum.
    Ineens,
    /// Vier termijnen, elk kwartaal één.
    Kwartaal,
    /// Twaalf termijnen, elke maand één.
    Maand,
}

impl Schedule {
    /// De ritmes die de simulator kent, met hun naam in een wereldbestand.
    const ALL: [(&'static str, Self); 3] = [
        ("ineens", Self::Ineens),
        ("kwartaal", Self::Kwartaal),
        ("maand", Self::Maand),
    ];

    /// Het ritme met deze naam, of `None` als er geen zo heet.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, schedule)| *schedule)
    }

    /// De naam waaronder dit ritme in een wereldbestand staat.
    pub fn name(self) -> &'static str {
        // Onbereikbaar leeg: elke variant staat in `ALL`.
        Self::ALL
            .iter()
            .find(|(_, known)| *known == self)
            .map_or("", |(name, _)| *name)
    }

    /// Komma-gescheiden opsomming van de ritmes, voor foutmeldingen.
    pub(crate) fn listing() -> String {
        Self::ALL
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Hoeveel termijnen dit ritme over een jaar kent.
    fn terms(self) -> u32 {
        match self {
            Self::Ineens => 1,
            Self::Kwartaal => 4,
            Self::Maand => 12,
        }
    }

    /// Hoeveel maanden er tussen twee termijnen zitten.
    fn step_months(self) -> u32 {
        match self {
            Self::Ineens => 0,
            Self::Kwartaal => 3,
            Self::Maand => 1,
        }
    }
}

/// De waarde van `vanaf` die naar de **bekendmaking** wijst in plaats van naar
/// een dag.
///
/// Platformvocabulaire, zoals de naam van een stage: het is niet een datum die
/// toevallig zo heet, maar de gebeurtenis waarna de termijn pas gaat lopen (Awb
/// 3:40 — een besluit treedt niet in werking voordat het is bekendgemaakt).
pub const VANAF_BEKENDMAKING: &str = "bekendmaking";

/// Vanaf wanneer de termijnen van één verplichting lopen.
///
/// Twee vormen, en het verschil is niet een datum meer of minder: bij de eerste
/// staat de vervaldag bij het besluit al vast, bij de tweede hangt ze aan een
/// gebeurtenis die nog moet plaatsvinden. Wie ze allebei als datum zou
/// behandelen, moest bij de tweede een dag verzinnen — en dan belooft het gram
/// een betaaldatum die de wet niet gegeven heeft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObligationStart {
    /// De eerste termijn vervalt op deze dag.
    Op(NaiveDate),
    /// De termijnen gaan pas lopen bij de bekendmaking van dit besluit.
    Bekendmaking,
}

/// Een verplichting die op de bekendmaking wacht.
///
/// Alles wat een termijn nodig heeft staat er al — het bedrag, de partijen, het
/// ritme, de grondslag en het volgnummer waarmee ze in het schema van dit
/// besluit valt — behalve de **vervaldatum**. Die volgt uit de uiterste
/// betaaldatum die de Awb bij de bekendmaking uitrekent, en die is er bij het
/// besluit nog niet.
///
/// Ze staat in het decretogram en niet in een register ernaast: wat een besluit
/// belooft, hoort in het gram van dat besluit te staan, ook als er nog geen dag
/// bij hoort. De bekendmaking leest haar daar terug en maakt er termijnen van.
#[derive(Debug, Clone, PartialEq)]
pub struct WachtendeVerplichting {
    /// Wat voor verplichting dit is.
    pub soort: ObligationKind,
    /// De partij die moet nakomen, met naam.
    pub schuldenaar: String,
    /// De partij aan wie nagekomen moet worden, met naam.
    pub schuldeiser: String,
    /// De cel die namens de schuldenaar nakomt, of `None`.
    pub betaler: Option<String>,
    /// Het hele bedrag van deze verplichting, vóór de verdeling over termijnen.
    pub bedrag: Decimal,
    /// Het ritme waarin ze straks vervalt.
    pub schedule: Schedule,
    /// Het volgnummer van haar eerste termijn binnen het schema van dit besluit.
    ///
    /// Vastgesteld bij het besluit en niet bij de bekendmaking: de volgnummers
    /// lopen door over álle verplichtingen van één besluit, dus ze staan vast
    /// zodra het schema er is. Zou de bekendmaking opnieuw beginnen te tellen,
    /// dan kreeg een termijn van deze verplichting hetzelfde nummer als een
    /// termijn die al vervallen is, en viel er bij het nakomen één stil weg.
    pub eerste_volgnummer: i64,
    /// Hoeveel termijnen het hele schema van dit besluit kent.
    pub termijnen: i64,
    /// Waarop deze verplichting berust, in de woorden van het lexogram.
    pub grondslag: String,
    /// Het artikel dat haar declareert, met de versie die toen gold.
    pub herkomst: ObligationOrigin,
    /// De uitkomst van de bekendmaking die de eerste vervaldag levert, zoals
    /// de verplichting haar noemt (`vervaldatum`).
    ///
    /// In het gram en niet opnieuw uit de wet gelezen bij de bekendmaking: wat
    /// het besluit beloofde, staat in het besluit, ook waaraan de dag straks
    /// ontleend wordt.
    pub vervaldatum_uit: String,
}

/// Veld van een wachtende verplichting met de uitkomst die haar vervaldag levert.
pub const VERVALDATUM_UIT: &str = "vervaldatum_uit";

impl WachtendeVerplichting {
    /// De wachtende verplichting als vastlegbare waarde, voor in het decretogram.
    fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            (
                SOORT.to_string(),
                Value::String(self.soort.name().to_string()),
            ),
            (
                SCHULDENAAR.to_string(),
                Value::String(self.schuldenaar.clone()),
            ),
            (
                SCHULDEISER.to_string(),
                Value::String(self.schuldeiser.clone()),
            ),
            (BETALER.to_string(), optional_text(self.betaler.as_deref())),
            (BEDRAG.to_string(), amount(self.bedrag)),
            (
                "schedule".to_string(),
                Value::String(self.schedule.name().to_string()),
            ),
            (VOLGNUMMER.to_string(), Value::Int(self.eerste_volgnummer)),
            ("termijnen".to_string(), Value::Int(self.termijnen)),
            (GRONDSLAG.to_string(), Value::String(self.grondslag.clone())),
            (LEXOGRAM.to_string(), self.herkomst.as_value()),
            (
                VERVALDATUM_UIT.to_string(),
                Value::String(self.vervaldatum_uit.clone()),
            ),
        ]))
    }

    /// Lees een wachtende verplichting terug uit het gram waarin ze staat.
    ///
    /// `None` zodra er iets niet staat of niet te lezen is. De aanroeper maakt er
    /// een weigering van met de zaak erbij: een bekendmaking die de helft van een
    /// verplichting zou inroosteren, is erger dan een bekendmaking die niet
    /// doorgaat.
    fn from_value(value: &Value) -> Option<Self> {
        let Value::Object(fields) = value else {
            return None;
        };
        let text = |name: &str| fields.get(name).and_then(Value::as_str).map(str::to_string);
        Some(Self {
            soort: ObligationKind::from_name(fields.get(SOORT).and_then(Value::as_str)?)?,
            schuldenaar: text(SCHULDENAAR)?,
            schuldeiser: text(SCHULDEISER)?,
            // Geen cel onder de schuldenaar is geen fout maar een stand van deze
            // wereld: de termijn wordt ingeroosterd en niet nagekomen.
            betaler: text(BETALER),
            bedrag: fields.get(BEDRAG).and_then(Value::as_decimal)?,
            schedule: Schedule::from_name(fields.get("schedule").and_then(Value::as_str)?)?,
            eerste_volgnummer: fields.get(VOLGNUMMER).and_then(Value::as_int)?,
            termijnen: fields.get("termijnen").and_then(Value::as_int)?,
            grondslag: text(GRONDSLAG)?,
            herkomst: ObligationOrigin::from_value(fields.get(LEXOGRAM)?)?,
            vervaldatum_uit: text(VERVALDATUM_UIT)?,
        })
    }

    /// De termijnen van deze verplichting, nu er een startdatum is.
    ///
    /// Dezelfde verdeling als bij een besluit dat zijn termijnen meteen
    /// inroostert ([`BesluitDefinition::schedule_obligations`]): het bedrag gaat
    /// in hele eenheden over de termijnen en het restant naar de laatste, en de
    /// volgnummers sluiten aan op het schema dat het besluit al had.
    ///
    /// `bekendmaking` is de dag van de bekendmaking: een termijn die daarvóór zou
    /// vervallen, wordt op die dag ingehaald (zie [`inhalen`]). Vóór de
    /// bekendmaking werkt het besluit niet, dus eerder betalen kan niet.
    pub(crate) fn termijnen_vanaf(
        &self,
        start: NaiveDate,
        bekendmaking: NaiveDate,
        zaak: &BesluitGram<'_>,
    ) -> Result<Vec<ObligationDue>> {
        let mut due = Vec::new();
        for (index, bedrag) in split(self.bedrag, self.schedule.terms())
            .into_iter()
            .enumerate()
        {
            // `index` telt de termijnen van dit ritme en komt nooit in de buurt
            // van de grens van u32.
            let step = u32::try_from(index).unwrap_or(u32::MAX);
            let vervaldatum = start
                .checked_add_months(Months::new(step * self.schedule.step_months()))
                .ok_or_else(|| SimulatorError::MalformedObligationDate {
                    cell: zaak.cell.to_string(),
                    besluit: zaak.besluit.to_string(),
                    template: VANAF_BEKENDMAKING.to_string(),
                    reason: format!(
                        "de termijn {} maanden na {start} valt buiten het bereik van de kalender",
                        step * self.schedule.step_months()
                    ),
                })?;
            let (vervaldatum, oorspronkelijke_vervaldatum) = inhalen(vervaldatum, bekendmaking);
            due.push(ObligationDue {
                soort: self.soort,
                schuldenaar: self.schuldenaar.clone(),
                schuldeiser: self.schuldeiser.clone(),
                betaler: self.betaler.clone(),
                decided_by: zaak.cell.to_string(),
                besluit: zaak.besluit.to_string(),
                zaakkenmerk: zaak.zaakkenmerk.to_string(),
                decided_op_moment: zaak.op_moment,
                schedule: self.schedule,
                vervaldatum,
                oorspronkelijke_vervaldatum,
                bedrag: amount(bedrag),
                grondslag: self.grondslag.clone(),
                herkomst: self.herkomst.clone(),
                volgnummer: self.eerste_volgnummer + i64::try_from(index).unwrap_or(i64::MAX),
                termijnen: self.termijnen,
                // De plek van het **besluit**, niet die van de bekendmaking: de
                // verplichting volgt uit het besluit, en daar hoort een betaling
                // ook naar terug te wijzen.
                besluit_gram: zaak.plek,
            });
        }
        Ok(due)
    }

    /// Leesbare regel voor een verslag en voor het beeld van de wereld.
    pub fn describe(&self) -> String {
        let langs = match &self.betaler {
            Some(cell) => format!("cel '{cell}'"),
            None => "geen cel in deze wereld: openstaand".to_string(),
        };
        format!(
            "{} van {} door {} aan {} ({}, {langs}) — wacht op de bekendmaking",
            self.soort.name(),
            amount(self.bedrag),
            self.schuldenaar,
            self.schuldeiser,
            self.schedule.name()
        )
    }
}

/// Een verplichting waarvan het bedrag op precies nul uitkwam: niets te betalen.
///
/// Wat er van een verplichting overblijft zonder termijnen: de soort, de
/// partijen, het ritme dat ze gehad zou hebben en de grondslag. Geen betaler en
/// geen volgnummer — er valt niets na te komen, dus er is geen cel die iets doet
/// en geen termijn om te nummeren. Zie [`NIETS_TE_BETALEN`] voor waarom ze in het
/// gram staat in plaats van eruit weg te vallen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NietsTeBetalen {
    /// Wat voor verplichting dit is. Nul keert niets om, dus altijd de soort
    /// die het lexogram declareert.
    pub soort: ObligationKind,
    /// De partij die had moeten nakomen, met naam.
    pub schuldenaar: String,
    /// De partij aan wie nagekomen had moeten worden, met naam.
    pub schuldeiser: String,
    /// Het ritme waarin ze vervallen was, als er iets te betalen was geweest.
    pub schedule: Schedule,
    /// Waarop deze verplichting berust, in de woorden van het lexogram.
    pub grondslag: String,
    /// Het artikel dat haar declareert, met de versie die toen gold.
    pub herkomst: ObligationOrigin,
}

impl NietsTeBetalen {
    /// Als vastlegbare waarde, voor in het decretogram.
    ///
    /// Met het bedrag erbij, ook al staat dat vast: `bedrag: 0` is wat een
    /// lezer van het gram zoekt, en een veld dat er alleen bij andere
    /// verplichtingen staat laat hem raden.
    fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            (
                SOORT.to_string(),
                Value::String(self.soort.name().to_string()),
            ),
            (
                SCHULDENAAR.to_string(),
                Value::String(self.schuldenaar.clone()),
            ),
            (
                SCHULDEISER.to_string(),
                Value::String(self.schuldeiser.clone()),
            ),
            (BEDRAG.to_string(), amount(Decimal::ZERO)),
            (
                "schedule".to_string(),
                Value::String(self.schedule.name().to_string()),
            ),
            (GRONDSLAG.to_string(), Value::String(self.grondslag.clone())),
            (LEXOGRAM.to_string(), self.herkomst.as_value()),
        ]))
    }

    /// Leesbare regel voor het journaal en een verslag.
    pub fn describe(&self) -> String {
        format!(
            "niets te betalen: {} door {} aan {} is op 0 vastgesteld, dus er wordt geen \
             termijn ingeroosterd (grondslag '{}', {})",
            self.soort.name(),
            self.schuldenaar,
            self.schuldeiser,
            self.grondslag,
            self.herkomst.describe()
        )
    }
}

/// Wat een besluit uit zijn verplichtingen inroostert, in drie delen.
///
/// Samen en niet als losse lijsten, want ze komen uit één schema: de volgnummers
/// en het aantal termijnen lopen over de eerste twee heen, en wat op nul uitviel
/// telt daar juist niet in mee.
#[derive(Debug, Clone, Default)]
pub(crate) struct Verplichtingenschema {
    /// De termijnen met een vervaldag.
    pub(crate) termijnen: Vec<ObligationDue>,
    /// De verplichtingen die op de bekendmaking wachten.
    pub(crate) wachtend: Vec<WachtendeVerplichting>,
    /// De verplichtingen waarvan het bedrag op nul uitkwam.
    pub(crate) niets_te_betalen: Vec<NietsTeBetalen>,
}

/// Het besluit waar een bekendmaking bij hoort, zoals het in de kroniek ligt.
///
/// De vier dingen die een termijn van dat besluit nodig heeft, bij elkaar: los
/// doorgegeven zijn het vier tekstjes die te verwisselen zijn.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BesluitGram<'a> {
    /// De cel die besloot, en in wiens kroniek het gram ligt.
    pub(crate) cell: &'a str,
    /// De besluit-definitie die het gram voortbracht.
    pub(crate) besluit: &'a str,
    /// Het zaakkenmerk van dat besluit.
    pub(crate) zaakkenmerk: &'a str,
    /// Het moment van dat besluit.
    pub(crate) op_moment: NaiveDate,
    /// De plek van dat gram in [`BESCHIKKINGEN`], geteld vanaf nul.
    pub(crate) plek: usize,
}

/// Eén termijn van één verplichting: wat er op een vervaldatum moet gebeuren.
///
/// Uitgerekend bij het besluit en vastgelegd in het decretogram, zodat te lezen
/// is wat er beloofd is voordat er iets betaald is. De verwijzing naar het
/// besluit reist mee: een betaling zonder de zaak, het besluit en het moment
/// waaruit ze volgt, is een bedrag zonder grondslag.
#[derive(Debug, Clone, PartialEq)]
pub struct ObligationDue {
    /// Wat voor verplichting dit is: een betaling of een terugvordering.
    pub soort: ObligationKind,
    /// De partij die moet nakomen, met naam.
    ///
    /// Een naam uit het recht — het bevoegd gezag dat de regeling aanwijst, of de
    /// waarde van de parameter die de declaratie noemt — en niet een cel. Wie er
    /// in déze wereld onder die naam betaalt, staat in [`Self::betaler`].
    pub schuldenaar: String,
    /// De partij aan wie nagekomen moet worden, met naam.
    pub schuldeiser: String,
    /// De cel die namens de schuldenaar nakomt, of `None`.
    ///
    /// `None` betekent: deze wereld kent geen cel voor die naam. Dat is geen fout
    /// — een terugvordering op een burger is een echte verplichting, ook in een
    /// wereld waarin die burger niet als cel meedoet. De klok roostert de termijn
    /// dan wel in en komt haar niet na, en het beeld laat haar openstaan.
    pub betaler: Option<String>,
    /// De cel die het besluit nam.
    pub decided_by: String,
    /// De besluit-definitie waaruit deze verplichting volgt.
    pub besluit: String,
    /// Het zaakkenmerk van dat besluit; hierop groepeert [`BETALINGEN`].
    pub zaakkenmerk: String,
    /// Het moment van dat besluit.
    pub decided_op_moment: NaiveDate,
    /// Het ritme waarin deze termijn valt.
    pub schedule: Schedule,
    /// De dag waarop deze termijn vervalt.
    ///
    /// Nooit vóór het moment waarop er betaald kan worden: bij een ingehaalde
    /// termijn is dit die dag, en staat de dag uit het schema in
    /// [`Self::oorspronkelijke_vervaldatum`].
    pub vervaldatum: NaiveDate,
    /// De dag uit het schema, als die vóór het besluit (of de bekendmaking) lag
    /// en de termijn daarom ingehaald wordt; anders `None`.
    ///
    /// Een te laat besluit blijft een besluit: wat er al betaald had moeten
    /// zijn, wordt op de dag dat het kan in één keer ingehaald, en niet
    /// weggelaten of met terugwerkende kracht vastgelegd.
    pub oorspronkelijke_vervaldatum: Option<NaiveDate>,
    /// Het bedrag van deze termijn.
    pub bedrag: Value,
    /// Het volgnummer binnen het schema van dít besluit, vanaf 1.
    ///
    /// Hierop plus de verwijzing naar het besluit is een betaling te herkennen:
    /// dezelfde zaak, hetzelfde besluit en hetzelfde volgnummer is dezelfde
    /// termijn, ook als de klok er in kleine stappen langs komt.
    ///
    /// Doorgenummerd over álle verplichtingen van het besluit, en niet per
    /// verplichting opnieuw. Dat moet ook: legt één besluit twee verplichtingen
    /// op, dan zouden twee termijnen met hetzelfde nummer bij het nakomen voor
    /// elkaar doorgaan en zou de tweede stil wegvallen.
    pub volgnummer: i64,
    /// Hoeveel termijnen het schema van dit besluit in totaal kent.
    ///
    /// Alleen om een termijn leesbaar te kunnen benoemen ("termijn 2 van 4"),
    /// en niet uit [`Self::schedule`] af te leiden: een besluit mag meer dan één
    /// verplichting opleggen, elk met een eigen ritme.
    pub termijnen: i64,
    /// Waarop deze verplichting berust, in de woorden van het lexogram.
    pub grondslag: String,
    /// Het artikel dat haar declareert, met de versie die toen gold.
    pub herkomst: ObligationOrigin,
    /// De plek van het decretogram in [`BESCHIKKINGEN`] van [`Self::decided_by`].
    ///
    /// Samen met die cel en die stroom is dit de verwijzing waarmee het beeld
    /// van de wereld een gram aanwijst, en dus de weg van een betaling terug naar
    /// het besluit en zijn uitvoeringstrace.
    ///
    /// Wordt gezet ná het vastleggen van dat gram, in [`crate::Cell::decide`]:
    /// het is de plek waar het gram terechtkwam en niet de plek waar het naar
    /// verwachting terecht zou komen.
    ///
    /// Tussen het inroosteren en dat vastleggen staat er nul, en nul is hier
    /// géén "nog niet ingevuld": het is de plek van het eerste gram in de
    /// stroom. Dat dit toch geen half antwoord kan opleveren, hangt aan twee
    /// dingen die samen in [`crate::Cell::decide`] staan: het schema wordt daar
    /// gemaakt én daar ingevuld, en het decretogram schrijft deze plek niet mee
    /// in zijn eigen gram (zie `ObligationDue::as_value`). Een verplichting
    /// bestaat hier dus nooit buiten het besluit dat haar oplegde om.
    pub besluit_gram: usize,
}

impl ObligationDue {
    /// De velden die beide kanten van een betaling vastleggen.
    ///
    /// Ook de twee partijen: wie er betaalde en aan wie, hoort in het feit te
    /// staan en niet alleen in het besluit waaruit het volgt. Een som over deze
    /// stroom gaat anders over bedragen waarvan de richting alleen elders staat.
    pub(crate) fn fields(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([
            (
                ZAAKKENMERK.to_string(),
                Value::String(self.zaakkenmerk.clone()),
            ),
            (BEDRAG.to_string(), self.bedrag.clone()),
            (VOLGNUMMER.to_string(), Value::Int(self.volgnummer)),
            (
                SOORT.to_string(),
                Value::String(self.soort.name().to_string()),
            ),
            (
                SCHULDENAAR.to_string(),
                Value::String(self.schuldenaar.clone()),
            ),
            (
                SCHULDEISER.to_string(),
                Value::String(self.schuldeiser.clone()),
            ),
            (BESLUIT.to_string(), Value::String(self.besluit.clone())),
            (
                BESLUIT_CEL.to_string(),
                Value::String(self.decided_by.clone()),
            ),
            (
                BESLUIT_KRONIEK.to_string(),
                Value::String(BESCHIKKINGEN.to_string()),
            ),
            (
                BESLUIT_GRAM.to_string(),
                Value::Int(i64::try_from(self.besluit_gram).unwrap_or(i64::MAX)),
            ),
            (
                BESLUIT_OP_MOMENT.to_string(),
                Value::String(self.decided_op_moment.to_string()),
            ),
        ])
    }

    /// De grondslag van beide vastleggingen: het recht, en het besluit waaruit
    /// ze volgen.
    ///
    /// De wet vooraan en het besluit erachter, want dat is de volgorde waarin ze
    /// gelden: het lexogram zegt dát er betaald moet worden, het besluit welke
    /// zaak en welke termijn dat is.
    fn grondslag(&self) -> String {
        format!(
            "{} ({}): {} uit besluit '{}' van cel '{}' ({}), door {} aan {}, termijn {} van {}",
            self.grondslag,
            self.herkomst.describe(),
            self.soort.name(),
            self.besluit,
            self.decided_by,
            self.decided_op_moment,
            self.schuldenaar,
            self.schuldeiser,
            self.volgnummer,
            self.termijnen
        )
    }

    /// Het executogram van de betalende cel: zij betaalde.
    ///
    /// De cel komt van de aanroeper en niet uit [`Self::betaler`], omdat alleen
    /// de aanroeper weet dat er er een cel is: staat die op `None`, dan is er
    /// niets vast te leggen en hoort er ook geen gram gemaakt te kunnen worden.
    pub(crate) fn payment_event(&self, betaler: &str) -> ChronicleEvent {
        ChronicleEvent {
            name: self.soort.gedaan(),
            intake: Intake::Betaling,
            recording_actor: betaler.to_string(),
            grondslag: self.grondslag(),
            op_moment: self.vervaldatum,
            fields: self.fields(),
        }
    }

    /// Het executogram van de besluitende cel: haar werd gemeld dat er betaald is.
    ///
    /// Een eigen vastlegging met een eigen kanaal, en geen kopie van het gram
    /// hiernaast: wat de betalende cel deed staat in háár kroniek, en wat deze
    /// cel overkwam in de hare. Zo weten beide kanten wat er gebeurde zonder dat
    /// er één staat is die twee cellen delen.
    pub(crate) fn delivery_event(&self) -> ChronicleEvent {
        ChronicleEvent {
            name: self.soort.gemeld(),
            intake: Intake::Levering,
            recording_actor: self.decided_by.clone(),
            grondslag: self.grondslag(),
            op_moment: self.vervaldatum,
            fields: self.fields(),
        }
    }

    /// Deze termijn als vastlegbare waarde, voor in het decretogram.
    ///
    /// Een ingehaalde termijn draagt er [`OORSPRONKELIJKE_VERVALDATUM`] bij.
    fn as_value(&self) -> Value {
        let mut fields = BTreeMap::from([
            (
                "vervaldatum".to_string(),
                Value::String(self.vervaldatum.to_string()),
            ),
            (BEDRAG.to_string(), self.bedrag.clone()),
            (VOLGNUMMER.to_string(), Value::Int(self.volgnummer)),
            (
                SOORT.to_string(),
                Value::String(self.soort.name().to_string()),
            ),
            (
                SCHULDENAAR.to_string(),
                Value::String(self.schuldenaar.clone()),
            ),
            (
                SCHULDEISER.to_string(),
                Value::String(self.schuldeiser.clone()),
            ),
            (BETALER.to_string(), optional_text(self.betaler.as_deref())),
            (
                "schedule".to_string(),
                Value::String(self.schedule.name().to_string()),
            ),
            (GRONDSLAG.to_string(), Value::String(self.grondslag.clone())),
            (LEXOGRAM.to_string(), self.herkomst.as_value()),
        ]);
        if let Some(oorspronkelijk) = self.oorspronkelijke_vervaldatum {
            fields.insert(
                OORSPRONKELIJKE_VERVALDATUM.to_string(),
                Value::String(oorspronkelijk.to_string()),
            );
        }
        Value::Object(fields)
    }

    /// Leesbare termijn voor een verslag.
    ///
    /// Wie aan wie, en langs welke cel dat gaat. Staat er geen cel onder de
    /// schuldenaar, dan zegt de regel dat ook: een termijn die niemand nakomt is
    /// iets anders dan een termijn waarover het verslag zwijgt.
    pub fn describe(&self) -> String {
        let langs = match &self.betaler {
            Some(cell) => format!("cel '{cell}'"),
            None => "geen cel in deze wereld: openstaand".to_string(),
        };
        let ingehaald = self
            .oorspronkelijke_vervaldatum
            .map(|oorspronkelijk| format!(", ingehaald (oorspronkelijk {oorspronkelijk})"))
            .unwrap_or_default();
        format!(
            "{} {}/{} van {} op {}{ingehaald}, door {} aan {} ({}, {langs})",
            self.soort.name(),
            self.volgnummer,
            self.termijnen,
            self.bedrag,
            self.vervaldatum,
            self.schuldenaar,
            self.schuldeiser,
            self.schedule.name()
        )
    }
}

/// De vier vormen die een input van een besluit kan hebben, voor foutmeldingen.
///
/// Eén tekst, want elke weigering hieronder somt ze op: wie er twee door elkaar
/// haalt, hoort in dezelfde melding te lezen wat de keuze was.
const INPUT_FORMS: &str = "een input komt uit een eigen kroniek (`from_chronicle` + `field`), \
                           uit een parameter (`param`), van een andere cel \
                           (`accept_from` + `lexostatus` + `field`), of uit een eerder \
                           besluit van dezelfde cel over dezelfde zaak \
                           (`from_decretogram` + `field`)";

/// De twee vormen die een partij van een verplichting kan hebben, voor
/// foutmeldingen.
///
/// Eén tekst, zoals [`INPUT_FORMS`]: wie er een derde vorm probeert, hoort in
/// dezelfde melding te lezen wat de keuze was.
const PARTY_FORMS: &str = "een partij is `#bevoegd_gezag` of `$parameter` van het besluit, en \
                           een naam die er letterlijk staat zou dezelfde regeling in elke \
                           wereld bij dezelfde organisatie leggen";

/// De verwijzing waarmee een `accept_from` het zaakkenmerk van het lopende
/// besluit meegeeft.
///
/// Eén ingebouwde naam naast `$parameter` en letterlijke tekst, en met opzet
/// niet een algemeen `{…}`-sjabloon: wat een cel over de grens meestuurt, hoort
/// te lezen als wat het is. Het kenmerk komt uit het eigen sjabloon van de
/// definitie en nooit uit een vrije parameter — anders zou een besluit onder de
/// vlag van "de zaak waarover ik nu besluit" naar de zaak van een ander kunnen
/// vragen.
const ZAAKKENMERK_REFERENCE: &str = "$zaakkenmerk";

/// Waar één input van een besluit vandaan komt.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "BesluitInputFields")]
pub enum BesluitInput {
    /// Uit een eigen kroniek van de besluitende cel (tier 1 van RFC-022 §4.2).
    ///
    /// De laatste vastlegging op of vóór het moment van het besluit, gezocht op
    /// het sleutelveld van die stroom — en de waarde van dat sleutelveld komt
    /// uit de gedocumenteerde parameter met dezelfde naam.
    FromChronicle {
        /// De eigen kroniekstroom waaruit gelezen wordt.
        chronicle: String,
        /// Het veld van die vastlegging dat de waarde draagt.
        field: String,
    },
    /// Meegegeven bij het besluit zelf.
    Param {
        /// De gedocumenteerde parameter die de waarde levert.
        param: String,
    },
    /// **Geaccepteerd** van een andere cel: die cel stelt de waarde vast, deze
    /// cel rekent haar niet na (RFC-009 beslisboom stap 4, invariant I5).
    ///
    /// De vraag gaat niet vanuit de cel: ze gaat langs de veiligheidscontext
    /// van de besluitende cel naar het transport, en die twee houdt een cel
    /// niet (RFC-022 §2). Wat de cel hier declareert is dus een *verzoek* —
    /// [`AcceptanceRequest`] — en wie het inwilligt staat buiten de cel.
    ///
    /// Wat terugkomt gaat als parameter de engine in en met haar herkomst het
    /// decretogram in ([`InputOrigin::Accepted`]); nergens anders. Een volgend
    /// besluit vraagt opnieuw.
    AcceptFrom {
        /// De cel die de waarde vaststelt.
        cell: String,
        /// De lexostatus die daar opgevraagd wordt.
        lexostatus: String,
        /// De uitkomst van die lexostatus die de waarde draagt.
        field: String,
        /// De parameters van die vraag, op de naam die de bevraagde cel
        /// documenteert. Een waarde `$naam` verwijst naar een gedocumenteerde
        /// parameter van dít besluit, `$zaakkenmerk` naar het zaakkenmerk van het
        /// lopende besluit; elke andere waarde is letterlijke tekst — dezelfde
        /// vorm als de parameters van een reductie.
        params: BTreeMap<String, String>,
    },
    /// Uit een **eerder besluit van dezelfde cel over dezelfde zaak**.
    ///
    /// De smalle tegenhanger van de weigering hiernaast: teruglezen mag, maar
    /// alleen benoemd. `from_chronicle: beschikkingen` blijft verboden (zie
    /// [`SimulatorError::DecretogramAsBesluitInput`]) — dat zou een besluit als
    /// eigen feit binnenhalen en de schaduwboekhouding van RFC-022 alsnog
    /// opleveren. Hier staat er letterlijk dát een eerder besluit teruggelezen
    /// wordt, en dat is ook wat het gram erover opschrijft
    /// ([`InputOrigin::EarlierDecretogram`]).
    ///
    /// De sleutel is altijd het zaakkenmerk van het **lopende** besluit, uit het
    /// eigen sjabloon: een vaststelling leest de verlening over dezelfde zaak
    /// terug, en nooit een zaak van een ander. Genomen wordt het laatste gram met
    /// deze besluitnaam en dat kenmerk op of vóór het moment van dit besluit;
    /// ligt er geen, dan valt het besluit om en wordt er niets vastgelegd.
    FromDecretogram {
        /// De besluit-definitie van dezelfde cel waarvan het gram teruggelezen
        /// wordt.
        besluit: String,
        /// Het veld van dat gram dat de waarde draagt: een uitkomst, een vast
        /// veld, of een van de inputs waarop dat besluit rekende.
        field: String,
    },
}

impl BesluitInput {
    /// De cel waarvan deze input geaccepteerd wordt, als dat er een is.
    ///
    /// Voor wie de afspraken van buiten wil nalopen — de wereld toetst ermee of
    /// de peer bestaat — zonder de vorm van de variant na te bouwen.
    pub fn accepts_from(&self) -> Option<&str> {
        Some(self.accepted_query()?.0)
    }

    /// De cel én de lexostatus die deze input bij een ander opvraagt.
    ///
    /// Eén tak van het toegestane vraaggraf, zoals de besluit-definitie hem
    /// aanwijst (invariant I3). De lexostatus hoort erbij: wat een cel
    /// publiceert zijn losse, gedocumenteerde namen (RFC-022 §4.1), dus "deze
    /// cel mag die cel bevragen" is als afspraak te grof.
    pub fn accepted_query(&self) -> Option<(&str, &str)> {
        match self {
            Self::AcceptFrom {
                cell, lexostatus, ..
            } => Some((cell, lexostatus)),
            Self::FromChronicle { .. } | Self::Param { .. } | Self::FromDecretogram { .. } => None,
        }
    }
}

/// Het YAML-oppervlak van een input: alle velden van alle vormen, los.
///
/// Zelfde keuze als bij [`crate::Reduction`]: de vorm valt hieronder en niet in
/// serde, zodat er in de foutmelding staat wat er mis is in plaats van "data did
/// not match any variant".
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BesluitInputFields {
    from_chronicle: Option<String>,
    field: Option<String>,
    param: Option<String>,
    accept_from: Option<String>,
    lexostatus: Option<String>,
    params: Option<BTreeMap<String, String>>,
    from_decretogram: Option<String>,
}

impl TryFrom<BesluitInputFields> for BesluitInput {
    type Error = String;

    fn try_from(fields: BesluitInputFields) -> std::result::Result<Self, Self::Error> {
        // Precies één van de vier ankers wijst de vorm aan. Twee ankers is geen
        // vorm met een extraatje maar een input waarvan niemand kan zeggen waar
        // ze vandaan komt, dus de melding noemt ze beide bij hun waarde.
        let anchors: Vec<(&str, &str)> = [
            ("from_chronicle", fields.from_chronicle.as_deref()),
            ("param", fields.param.as_deref()),
            ("accept_from", fields.accept_from.as_deref()),
            ("from_decretogram", fields.from_decretogram.as_deref()),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .collect();

        let (kind, value) = match anchors.as_slice() {
            [] => return Err(format!("input noemt geen bron: {INPUT_FORMS}")),
            [single] => *single,
            named => {
                let listed = named
                    .iter()
                    .map(|(key, value)| format!("`{key}` '{value}'"))
                    .collect::<Vec<_>>()
                    .join(" en ");
                return Err(format!("input noemt {listed}; {INPUT_FORMS}"));
            }
        };

        match kind {
            "from_chronicle" => {
                reject_unused(kind, fields.lexostatus.is_some(), "`lexostatus`")?;
                reject_unused(kind, fields.params.is_some(), "`params`")?;
                let field = fields.field.ok_or_else(|| {
                    format!(
                        "input uit kroniekstroom '{value}' mist `field`: zonder veld \
                         weet de cel niet welke waarde van de vastlegging ze bedoelt"
                    )
                })?;
                Ok(Self::FromChronicle {
                    chronicle: value.to_string(),
                    field,
                })
            }
            "from_decretogram" => {
                reject_unused(kind, fields.lexostatus.is_some(), "`lexostatus`")?;
                reject_unused(kind, fields.params.is_some(), "`params`")?;
                let field = fields.field.ok_or_else(|| {
                    format!(
                        "input uit eerder besluit '{value}' mist `field`: zonder veld \
                         weet de cel niet welke waarde van dat gram ze bedoelt"
                    )
                })?;
                Ok(Self::FromDecretogram {
                    besluit: value.to_string(),
                    field,
                })
            }
            "accept_from" => {
                let lexostatus = fields.lexostatus.ok_or_else(|| {
                    format!(
                        "input die van cel '{value}' geaccepteerd wordt mist `lexostatus`: \
                         een cel is alleen te bevragen langs een naam die ze publiceert"
                    )
                })?;
                let field = fields.field.ok_or_else(|| {
                    format!(
                        "input die van cel '{value}' geaccepteerd wordt mist `field`: \
                         een lexostatus levert de uitkomsten die ze publiceert, en \
                         zonder veld weet de cel niet welke daarvan ze bedoelt"
                    )
                })?;
                Ok(Self::AcceptFrom {
                    cell: value.to_string(),
                    lexostatus,
                    field,
                    params: fields.params.unwrap_or_default(),
                })
            }
            // Onbereikbaar: de lijst hierboven kent geen vierde anker.
            _ => {
                reject_unused(kind, fields.field.is_some(), "`field`")?;
                reject_unused(kind, fields.lexostatus.is_some(), "`lexostatus`")?;
                reject_unused(kind, fields.params.is_some(), "`params`")?;
                Ok(Self::Param {
                    param: value.to_string(),
                })
            }
        }
    }
}

/// Weiger een veld dat bij een andere vorm hoort dan de gekozen.
///
/// Stil laten liggen zou erger zijn dan streng zijn: een `lexostatus` naast een
/// `param` leest als een vraag over de celgrens en is er geen.
fn reject_unused(kind: &str, present: bool, field: &str) -> std::result::Result<(), String> {
    if present {
        return Err(format!(
            "{field} hoort niet bij een input met `{kind}`; {INPUT_FORMS}"
        ));
    }
    Ok(())
}

/// Eén waarde die een besluit bij een andere cel gaat ophalen.
///
/// De cel stelt dit verzoek samen en zet het **niet** zelf door: ze houdt geen
/// veiligheidscontext en geen transport (RFC-022 §2). Wie het inwilligt — in
/// deze opstelling [`crate::World::decide`] — geeft de waarde met haar herkomst
/// terug aan het besluit.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptanceRequest {
    /// De input van het besluit die met deze waarde gevuld wordt.
    pub input: String,
    /// De cel aan wie gevraagd wordt.
    pub cell: String,
    /// De lexostatus die daar opgevraagd wordt.
    pub lexostatus: String,
    /// De uitkomst van die lexostatus die de waarde draagt.
    pub field: String,
    /// De parameters van de vraag, met de verwijzingen al ingevuld.
    pub params: BTreeMap<String, Value>,
}

/// Waar één waarde in een decretogram vandaan kwam.
///
/// Dit is wat het decretogram zelf draagt: niet alleen de waarde waarop besloten
/// is, maar ook wie haar aanleverde en van wanneer ze was. Zonder die herkomst
/// is een besluit niet terug te lezen, en is "accepteren in plaats van
/// narekenen" niet van "gokken" te onderscheiden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputOrigin {
    /// Uit een eigen kroniek van de besluitende cel.
    OwnChronicle {
        /// De stroom waaruit gelezen is.
        chronicle: String,
        /// Het veld dat de waarde droeg.
        field: String,
        /// Het moment van de vastlegging waaruit de waarde komt — niet het
        /// moment van het besluit. Dat verschil is het hele punt van een
        /// kroniek.
        recorded_op_moment: NaiveDate,
    },
    /// Meegegeven bij het besluit.
    Parameter {
        /// De parameter die de waarde leverde.
        parameter: String,
    },
    /// **Geaccepteerd** van een andere cel, en dus niet hier uitgerekend
    /// (invariant I5).
    ///
    /// Dit is het bewijsstuk van die ene vraag, uitgeschreven in gewone velden:
    /// bij wie, onder welke naam, op welk moment, en ondertekend door wie. Niet
    /// als geleend type uit de veiligheidscontext — een cel kent die niet — maar
    /// als wat het gram draagt en een lezer later nakijkt.
    Accepted {
        /// De cel die de waarde vaststelde.
        cell: String,
        /// Het bevoegd gezag dat die cel bij haar antwoord noemde, als zij dat
        /// doet (`competent_authority` in de uitkomsten van haar lexostatus).
        ///
        /// De cel is een adres, dit is een gezag, en voor RFC-013
        /// `accepted_values` is het tweede wat telt: wie het terugleest hoort te
        /// zien wiens vaststelling geaccepteerd is en niet alleen bij welk
        /// systeem ze opgehaald is. `None` betekent dat de bevraagde lexostatus
        /// er niets over publiceert — een gat bij de bron, en geen uitnodiging
        /// om het hier in te vullen.
        authority: Option<String>,
        /// De lexostatus waaronder ze dat publiceert.
        lexostatus: String,
        /// De uitkomst van die lexostatus die de waarde droeg.
        field: String,
        /// Het moment waarop het antwoord geldt.
        op_moment: NaiveDate,
        /// De identiteit die de vraag stelde en ondertekende.
        asked_by: String,
        /// De (gesimuleerde) ondertekening van die vraag.
        signature: String,
    },
    /// Teruggelezen uit een **eerder besluit** van dezelfde cel over dezelfde
    /// zaak.
    ///
    /// Een eigen variant en niet [`Self::OwnChronicle`] met de stroom
    /// `beschikkingen` erin: wie het gram leest, hoort te zien dát er een besluit
    /// is teruggelezen. Het is geen eigen feit — er is niets nieuws vastgesteld —
    /// en geen herberekening: het bedrag komt uit het gram zoals het toen
    /// vastgelegd is, onder het recht dat toen gold.
    EarlierDecretogram {
        /// De besluit-definitie waarvan het gram gelezen is.
        besluit: String,
        /// De zaak waarover beide besluiten gaan; hetzelfde kenmerk, per
        /// definitie.
        zaakkenmerk: String,
        /// Het moment van dat eerdere besluit — niet dat van dit besluit.
        moment: NaiveDate,
    },
}

impl InputOrigin {
    /// De herkomst als vastlegbare waarde, voor in het decretogram.
    ///
    /// `pub(crate)`: het journaal draagt de herkomst van elke input in precies
    /// deze vorm (zie [`crate::journal::ExecutedInput`]), en het beeld van de
    /// wereld geeft haar ook zo door ([`crate::snapshot::FieldOrigin`]). Eén
    /// vocabulaire voor één ding: wie hier een tweede schrijfwijze naast zet,
    /// laat twee lezers van hetzelfde gram verschillende woorden zien.
    pub(crate) fn as_value(&self) -> Value {
        match self {
            Self::OwnChronicle {
                chronicle,
                field,
                recorded_op_moment,
            } => Value::Object(BTreeMap::from([
                (
                    "herkomst".to_string(),
                    Value::String("eigen_kroniek".to_string()),
                ),
                ("chronicle".to_string(), Value::String(chronicle.clone())),
                ("field".to_string(), Value::String(field.clone())),
                (
                    "op_moment".to_string(),
                    Value::String(recorded_op_moment.to_string()),
                ),
            ])),
            Self::Parameter { parameter } => Value::Object(BTreeMap::from([
                (
                    "herkomst".to_string(),
                    Value::String("parameter".to_string()),
                ),
                ("parameter".to_string(), Value::String(parameter.clone())),
            ])),
            Self::Accepted {
                cell,
                authority,
                lexostatus,
                field,
                op_moment,
                asked_by,
                signature,
            } => Value::Object(BTreeMap::from([
                (
                    "herkomst".to_string(),
                    Value::String("geaccepteerd".to_string()),
                ),
                ("cell".to_string(), Value::String(cell.clone())),
                (
                    COMPETENT_AUTHORITY.to_string(),
                    optional_text(authority.as_deref()),
                ),
                ("lexostatus".to_string(), Value::String(lexostatus.clone())),
                ("field".to_string(), Value::String(field.clone())),
                (
                    "op_moment".to_string(),
                    Value::String(op_moment.to_string()),
                ),
                ("asked_by".to_string(), Value::String(asked_by.clone())),
                ("signature".to_string(), Value::String(signature.clone())),
            ])),
            Self::EarlierDecretogram {
                besluit,
                zaakkenmerk,
                moment,
            } => Value::Object(BTreeMap::from([
                (
                    "herkomst".to_string(),
                    Value::String("eerder_besluit".to_string()),
                ),
                (BESLUIT.to_string(), Value::String(besluit.clone())),
                (ZAAKKENMERK.to_string(), Value::String(zaakkenmerk.clone())),
                ("moment".to_string(), Value::String(moment.to_string())),
            ])),
        }
    }

    /// Is deze waarde van een andere cel geaccepteerd in plaats van hier
    /// uitgerekend?
    ///
    /// Het onderscheid van invariant I5, op één plek: een scenario, een verslag
    /// en de invarianten-gate stellen alle drie deze vraag, en ze horen hem niet
    /// elk op hun eigen manier te stellen.
    pub fn accepted_from(&self) -> Option<&str> {
        match self {
            Self::Accepted { cell, .. } => Some(cell),
            Self::OwnChronicle { .. }
            | Self::Parameter { .. }
            | Self::EarlierDecretogram { .. } => None,
        }
    }

    /// Is deze waarde teruggelezen uit een eerder besluit, en uit welk?
    ///
    /// De tegenhanger van [`Self::accepted_from`], en om dezelfde reden hier:
    /// teruglezen is de derde mogelijkheid naast "van een ander geaccepteerd" en
    /// "hier zelf vastgesteld". Zonder deze vraag zou een scenario een
    /// teruggelezen waarde alleen als "niet geaccepteerd" kunnen aanmerken, en
    /// dat is precies het etiket dat te ruim zit.
    pub fn earlier_besluit(&self) -> Option<&str> {
        match self {
            Self::EarlierDecretogram { besluit, .. } => Some(besluit),
            Self::OwnChronicle { .. } | Self::Parameter { .. } | Self::Accepted { .. } => None,
        }
    }

    /// Leesbare herkomst voor een verslag.
    pub fn describe(&self) -> String {
        match self {
            Self::OwnChronicle {
                chronicle,
                field,
                recorded_op_moment,
            } => format!("eigen kroniek '{chronicle}.{field}', vastgelegd {recorded_op_moment}"),
            Self::Parameter { parameter } => format!("parameter '{parameter}'"),
            Self::Accepted {
                cell,
                authority,
                lexostatus,
                field,
                op_moment,
                asked_by,
                signature,
            } => {
                // Het gezag erbij zodra de bron het noemt, en anders niets: een
                // verslag hoort niet "bevoegd gezag: onbekend" te zeggen over een
                // lexostatus die er nooit iets over beloofd heeft.
                let gezag = match authority {
                    Some(authority) => format!(", bevoegd gezag {authority}"),
                    None => String::new(),
                };
                format!(
                    "geaccepteerd van cel '{cell}'{gezag} ({lexostatus}.{field} op \
                     {op_moment}), gevraagd door {asked_by} [{signature}]"
                )
            }
            Self::EarlierDecretogram {
                besluit,
                zaakkenmerk,
                moment,
            } => format!("uit eerder besluit '{besluit}' over zaak '{zaakkenmerk}' ({moment})"),
        }
    }
}

/// Eén regeling die bij dit besluit werkelijk uitgevoerd is, met de versie die
/// op het moment van het besluit gold.
///
/// Naast [`Decretogram::regulation`] en niet in plaats ervan: die noemt de
/// regeling waarop het besluit *gaat*, terwijl een uitvoering er meer kan
/// aanroepen — een uitvoeringsregeling die een bedrag levert, een kaderwet die
/// een begrip invult (RFC-007). Wie alleen de eerste noemt, vertelt het halve
/// verhaal; wie alles noemt wat de cel geladen heeft, noemt ook recht dat deze
/// uitvoering niet geraakt heeft.
///
/// **Uitgevoerd, niet geladen.** Het receipt draagt met `scope.loaded_regulations`
/// elke versie die klaarstond, inclusief een versie die op dit moment niet gold.
/// Wat hier staat, is per regeling de ene versie waaronder er gerekend is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutedRegulation {
    /// De uitgevoerde regeling, bij `$id`.
    pub regulation: String,
    /// De `valid_from` van de versie die op het moment van het besluit gold;
    /// `None` als die versie geen versiedatum draagt.
    pub valid_from: Option<String>,
}

impl ExecutedRegulation {
    /// Leesbare regel voor een verslag: de regeling met haar versie.
    pub fn describe(&self) -> String {
        match &self.valid_from {
            Some(valid_from) => format!("{} {valid_from}", self.regulation),
            None => self.regulation.clone(),
        }
    }

    /// Deze regeling als vastlegbare waarde, voor in het decretogram.
    fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            (
                REGULATION.to_string(),
                Value::String(self.regulation.clone()),
            ),
            (
                REGULATION_VALID_FROM.to_string(),
                optional_text(self.valid_from.as_deref()),
            ),
        ]))
    }
}

/// Eén eigen kroniek die bij een uitvoering als databron klaarstond, met haar
/// stand op dat moment.
///
/// RFC-022 §1.3: draagt een kroniekstroom bij aan een uitvoering, dan worden
/// haar inhoud en versie vastgelegd zodat de uitvoering te reproduceren is. Voor
/// een kroniek is "de versie" haar stand op `op_moment` — alles wat toen
/// vastlag — en "de inhoud" een hash over precies die grammen. Élke stroom die
/// klaarstond staat erin, ook een lege (`grams: 0`): dat er niets lag, is net
/// zo goed een stand waarop de uitvoering leunde. Welke waarden de engine
/// werkelijk las, staat per stuk in de trace van het receipt; dit zegt waaruit
/// ze gelezen konden worden, en of dat nog dezelfde stroom is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChronicleSource {
    /// De eigen stroom die als databron klaarstond.
    pub chronicle: String,
    /// Het moment waarop de stand genomen is: het moment van het besluit.
    pub op_moment: NaiveDate,
    /// Hoeveel grammen er op dat moment in de stroom lagen.
    pub grams: usize,
    /// Een hash over die grammen, in de volgorde van de tijdas.
    pub content_hash: String,
}

impl ChronicleSource {
    /// Deze bron als vastlegbare waarde, voor in het decretogram.
    fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "chronicle".to_string(),
                Value::String(self.chronicle.clone()),
            ),
            (
                "op_moment".to_string(),
                Value::String(self.op_moment.to_string()),
            ),
            (
                "grams".to_string(),
                Value::Int(i64::try_from(self.grams).unwrap_or(i64::MAX)),
            ),
            (
                "content_hash".to_string(),
                Value::String(self.content_hash.clone()),
            ),
        ]))
    }
}

/// Eén vervulde afwijzingsvoorwaarde: waarop dit besluit afketste.
///
/// De grond staat in het gram en niet alleen in een melding ernaast: een
/// afwijzing die niet zegt waaróp ze afketste, is een besluit zonder motivering,
/// en dan is er over het gram niets terug te vragen. Het **artikel** hoort
/// erbij om dezelfde reden als bij het bevoegd gezag (RFC-002): een uitkomst is
/// een naam, een artikel is een grondslag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Afwijzingsgrond {
    /// De uitkomst van de regeling die de voorwaarde vervulde.
    pub output: String,
    /// De waarde die tot afwijzing leidt, en die deze uitkomst ook had.
    ///
    /// In het gram en niet weggelaten als "vanzelfsprekend onwaar": een
    /// voorwaarde mag ook op `true` staan — "is uitgesloten van de regeling" —
    /// en dan zegt een gram dat de waarde verzwijgt het omgekeerde van wat er
    /// gebeurd is.
    pub value: bool,
    /// Het artikel dat deze uitkomst voortbrengt, in de versie die op het moment
    /// van het besluit gold; `None` als er geen artikel bij te vinden was.
    pub article: Option<String>,
}

impl Afwijzingsgrond {
    /// Deze grond als vastlegbare waarde, voor in het decretogram.
    fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            ("output".to_string(), Value::String(self.output.clone())),
            ("value".to_string(), Value::Bool(self.value)),
            (
                "article".to_string(),
                optional_text(self.article.as_deref()),
            ),
        ]))
    }

    /// Leesbare grond voor een verslag.
    pub fn describe(&self) -> String {
        match &self.article {
            Some(article) => format!("{} is {} (artikel {article})", self.output, self.value),
            None => format!("{} is {}", self.output, self.value),
        }
    }
}

/// Eén input van een besluit: de waarde waarop besloten is, met haar herkomst.
#[derive(Debug, Clone, PartialEq)]
pub struct DecretogramInput {
    /// De waarde zoals ze meedeed in het besluit.
    pub value: Value,
    /// Waar ze vandaan kwam.
    pub origin: InputOrigin,
}

/// Het vastgelegde besluit: het RFC-013 Execution Receipt plus moment en
/// zaakkenmerk.
///
/// Geen parallel formaat naast het receipt (RFC-022 §1.2 — een decretogram *is*
/// een engine-uitkomst met een rechtskarakter, en het receipt is haar lichaam).
/// Wat erbij komt, is wat het receipt niet kan weten: op welk moment in de
/// logische tijd dit besluit genomen is, onder welk zaakkenmerk het terug te
/// vinden is, en waar elke input vandaan kwam.
#[derive(Debug, Clone)]
pub struct Decretogram {
    /// De cel die besloot.
    pub cell: String,
    /// De besluit-definitie die uitgevoerd is.
    pub besluit: String,
    /// Waaronder deze zaak terug te vinden is.
    pub zaakkenmerk: String,
    /// Het moment waarop besloten is.
    pub op_moment: NaiveDate,
    /// De uitgevoerde regeling, bij `$id`.
    pub regulation: String,
    /// De `valid_from` van de regelingversie die op `op_moment` gold.
    ///
    /// Dit is wat een besluit van een lexogram onderscheidt: het gram houdt vast
    /// welk recht gold toen, ook als er later een andere versie in werking treedt.
    pub regulation_valid_from: Option<String>,
    /// Het bevoegd gezag dat de regeling noemt (RFC-002). Een juridisch feit van
    /// het besluit, geen eigenschap van de cel (RFC-022 §2).
    ///
    /// `None` betekent dat de regeling er niets over zegt. Dat is een gat in die
    /// regeling en geen uitnodiging om het hier in te vullen: het gram zegt dan
    /// letterlijk dat er niets aangewezen is, en de wereld waarschuwt erover.
    pub competent_authority: Option<String>,
    /// De identiteit van de cel die besloot.
    ///
    /// Gelijk aan [`Self::competent_authority`] zodra die er is — een besluit
    /// door iemand anders wordt geweigerd — en dat ze allebei in het gram staan
    /// is met opzet: wie het terugleest, hoort te zien wie besloot zonder het
    /// uit de afwezigheid van een weigering af te moeten leiden.
    pub besloten_door: String,
    /// Het rechtskarakter dat de regeling aan deze uitkomst geeft
    /// (`produces.legal_character`). Altijd [`BESCHIKKING`]: dat is wat een
    /// decretogram tot een decretogram maakt (RFC-022 §1.2), en het optuigen én
    /// het besluit weigeren elke andere waarde. Dat het er toch als veld staat,
    /// is zodat het gram zélf zegt wat het is en een lezer het niet uit een
    /// afwezige weigering hoeft af te leiden.
    pub legal_character: String,
    /// Het besluittype van dit gram.
    ///
    /// [`AFWIJZING`] zodra een afwijzingsvoorwaarde vervuld was; anders wat het
    /// artikel dat de aansturende uitkomst voortbrengt in `produces.decision_type`
    /// aanwijst, en `None` als het daar niets over zegt.
    ///
    /// Anders dan [`Self::legal_character`] staat hier dus niet altijd hetzelfde:
    /// een beschikking omvat volgens Awb 1:3 lid 2 ook de afwijzing van de
    /// aanvraag, en dit veld is het enige dat de twee uit elkaar houdt.
    pub decision_type: Option<String>,
    /// De afwijzingsvoorwaarden die vervuld waren; leeg bij elk ander besluit.
    ///
    /// Is deze lijst niet leeg, dan is [`Self::decision_type`] [`AFWIJZING`] en
    /// is [`Self::obligations`] leeg. Die drie horen bij elkaar en worden op één
    /// plek gezet ([`crate::Cell::decide`]), zodat er geen gram kan bestaan dat
    /// afwijst en tegelijk iets belooft.
    pub afwijzingsgronden: Vec<Afwijzingsgrond>,
    /// De uitkomsten van het besluit: de aansturende uitkomst plus wat de
    /// definitie erbij noemt. Alle samen in één gram.
    ///
    /// Ook bij een afwijzing: wat de regeling wél geleverd heeft, hoort in het
    /// gram. Een weigering die haar eigen uitkomsten weglaat, is niet na te
    /// lopen.
    pub outputs: BTreeMap<String, Value>,
    /// De inputs waarop besloten is, met hun herkomst.
    pub inputs: BTreeMap<String, DecretogramInput>,
    /// De verplichtingen die uit dit besluit volgen, uitgerekend tot termijnen.
    ///
    /// Het schema hoort bij het besluit en niet bij de betaling: wat beloofd is,
    /// staat er vóórdat er iets betaald is, en verandert niet meer doordat er
    /// betaald wordt. Leeg als het besluit niets toekent.
    pub obligations: Vec<ObligationDue>,
    /// De verplichtingen die op de bekendmaking van dit besluit wachten.
    ///
    /// Wat de wet oplegt met `vanaf: bekendmaking`: het bedrag en de partijen
    /// staan vast, de vervaldatum nog niet. Zolang deze lijst niet leeg is, is er
    /// iets beloofd dat de klok niet nakomt — en dat hoort in het gram te staan
    /// en niet alleen in het beeld, want anders zou het uit de kroniek niet te
    /// zien zijn dat er nog iets openstaat.
    pub wacht_op_bekendmaking: Vec<WachtendeVerplichting>,
    /// De verplichtingen waarvan het bedrag op nul uitkwam; zie
    /// [`NIETS_TE_BETALEN`].
    pub niets_te_betalen: Vec<NietsTeBetalen>,
    /// De regelingen die deze uitvoering werkelijk uitvoerde, met de versie die
    /// op `op_moment` gold.
    ///
    /// De regeling van het besluit vooraan — díe uitvoering *is* het besluit —
    /// en daarachter elke regeling die er een input voor leverde (tier 2,
    /// RFC-022 §4.2).
    ///
    /// Vastgelegd in het gram onder [`EXECUTED_REGULATIONS`], zoals
    /// [`Self::chronicle_sources`] en om dezelfde reden: het is alleen tijdens
    /// de uitvoering bekend, het hoort bij dit besluit, en het is niet uit het
    /// receipt af te leiden — dat noemt wel elke *geladen* regeling en de
    /// herkomst van elke *uitkomst*, maar niet welke regeling welke input
    /// leverde. Zou het alleen in het journaal staan, dan vertelde het verhaal
    /// iets wat in geen enkele kroniek ligt (zie [`crate::journal::Execution`]).
    pub executed_regulations: Vec<ExecutedRegulation>,
    /// De eigen kronieken die als databron klaarstonden, met hun stand op het
    /// moment van het besluit (RFC-022 §1.3).
    ///
    /// Wat de engine daaruit las, staat niet in [`Self::inputs`] — die draagt
    /// alleen wat de besluit-definitie zelf aanleverde — maar in de trace van
    /// het receipt. Zonder deze lijst is niet te zeggen op welke stand van welke
    /// kroniek de uitvoering leunde, en dan is het besluit niet te reproduceren.
    pub chronicle_sources: Vec<ChronicleSource>,
    /// De hooks die op dit besluit vuurden maar niet konden draaien, omdat een
    /// input die ze nodig hebben er bij dit besluit niet is.
    pub hooks_niet_uitgevoerd: Vec<HookNietUitgevoerd>,
    /// Het volledige receipt van de uitvoering.
    pub receipt: ExecutionReceipt,
}

impl Decretogram {
    /// Is dit besluit een afwijzing?
    ///
    /// Op de gronden en niet op [`Self::decision_type`]: het type is wat er in
    /// het gram komt te staan, de gronden zijn waarom. Zouden de twee ooit
    /// uiteen kunnen lopen, dan hoort de vraag "wijst dit besluit af?" bij de
    /// reden te liggen en niet bij het etiket.
    pub fn is_afwijzing(&self) -> bool {
        !self.afwijzingsgronden.is_empty()
    }

    /// De waarden waarop een scenario dit besluit mag afrekenen.
    ///
    /// De uitkomsten van de regeling, plus het besluittype onder zijn eigen
    /// naam. Dat laatste hoort erbij omdat een afwijzing zich juist níet in een
    /// uitkomst laat aflezen: `heeft_recht… = false` is ook de uitkomst van een
    /// besluit dat het platform niet als afwijzing kent, en dan bewijst een
    /// verwachting over die uitkomst niets over de vorm van het gram.
    ///
    /// Alleen dit ene vaste veld erbij en niet alle: de rest — het receipt, de
    /// inputs, de verplichtingen — heeft eigen verwachtingen die meer zeggen dan
    /// een gelijkheidstoets op een waarde.
    pub fn assertable(&self) -> BTreeMap<String, Value> {
        let mut values = self.outputs.clone();
        values.insert(
            DECISION_TYPE.to_string(),
            optional_text(self.decision_type.as_deref()),
        );
        values
    }

    /// Elke waarde in dit gram die van een andere cel **geaccepteerd** is, op
    /// naam, met de cel die haar vaststelde.
    ///
    /// Twee wegen komen hier samen, en dat is met opzet één lijst: een input die
    /// de besluit-definitie met `accept_from` vulde, en een waarde die de
    /// *regeling* via een `source.regulation` naar een cel haalde (tier 3, die
    /// de engine in `accepted_values` van het receipt zet). Voor de vraag van
    /// invariant I5 — is dit narekenen of accepteren? — zijn ze hetzelfde, en
    /// wie ze apart houdt, controleert er straks maar één.
    pub fn accepted_values(&self) -> BTreeMap<&str, &str> {
        let from_inputs = self
            .inputs
            .iter()
            .filter_map(|(name, input)| Some((name.as_str(), input.origin.accepted_from()?)));
        let from_receipt = self
            .receipt
            .accepted_values
            .iter()
            .map(|accepted| (accepted.output.as_str(), accepted.authority.as_str()));
        from_inputs.chain(from_receipt).collect()
    }

    /// Elke waarde in dit gram die uit een **eerder besluit** van deze cel is
    /// teruggelezen, op naam, met de besluit-definitie waar ze vandaan komt.
    ///
    /// Naast [`Self::accepted_values`] en niet erin: geaccepteerd komt van een
    /// andere organisatie, teruggelezen uit de eigen kroniek. Wat de twee delen
    /// is dat het besluit ze geen van beide zelf heeft vastgesteld, en dat is
    /// wat een scenario per waarde moet kunnen aanwijzen.
    pub fn read_back_values(&self) -> BTreeMap<&str, &str> {
        self.inputs
            .iter()
            .filter_map(|(name, input)| Some((name.as_str(), input.origin.earlier_besluit()?)))
            .collect()
    }

    /// Het decretogram als kroniekgebeurtenis.
    ///
    /// Een gewoon executogram met `intake: eigen_besluit`, zodat de tijdreductie
    /// er zonder speciale gevallen overheen werkt: wie op een moment vóór dit
    /// besluit vraagt, ziet het niet, en wie erna vraagt krijgt het terug zoals
    /// het toen vastgelegd is.
    pub(crate) fn event(&self) -> Result<ChronicleEvent> {
        let mut fields: BTreeMap<String, Value> = BTreeMap::from([
            (
                ZAAKKENMERK.to_string(),
                Value::String(self.zaakkenmerk.clone()),
            ),
            (BESLUIT.to_string(), Value::String(self.besluit.clone())),
            (
                REGULATION.to_string(),
                Value::String(self.regulation.clone()),
            ),
            (
                REGULATION_VALID_FROM.to_string(),
                optional_text(self.regulation_valid_from.as_deref()),
            ),
            (
                EXECUTED_REGULATIONS.to_string(),
                Value::Array(
                    self.executed_regulations
                        .iter()
                        .map(ExecutedRegulation::as_value)
                        .collect(),
                ),
            ),
            (
                COMPETENT_AUTHORITY.to_string(),
                optional_text(self.competent_authority.as_deref()),
            ),
            (
                BESLOTEN_DOOR.to_string(),
                Value::String(self.besloten_door.clone()),
            ),
            (
                LEGAL_CHARACTER.to_string(),
                Value::String(self.legal_character.clone()),
            ),
            (
                DECISION_TYPE.to_string(),
                optional_text(self.decision_type.as_deref()),
            ),
            (
                AFWIJZINGSGROND.to_string(),
                Value::Array(
                    self.afwijzingsgronden
                        .iter()
                        .map(Afwijzingsgrond::as_value)
                        .collect(),
                ),
            ),
            (INPUTS.to_string(), inputs_value(&self.inputs)),
            (
                HOOK_NIET_UITGEVOERD.to_string(),
                hooks_niet_uitgevoerd_value(&self.hooks_niet_uitgevoerd),
            ),
            (
                OBLIGATIONS.to_string(),
                Value::Array(
                    self.obligations
                        .iter()
                        .map(ObligationDue::as_value)
                        .collect(),
                ),
            ),
            (
                WACHT_OP_BEKENDMAKING.to_string(),
                Value::Array(
                    self.wacht_op_bekendmaking
                        .iter()
                        .map(WachtendeVerplichting::as_value)
                        .collect(),
                ),
            ),
            (
                NIETS_TE_BETALEN.to_string(),
                Value::Array(
                    self.niets_te_betalen
                        .iter()
                        .map(NietsTeBetalen::as_value)
                        .collect(),
                ),
            ),
            // De stage waarin dit gram ontstond (RFC-008). Een decretogram *is*
            // het besluit, dus hier staat altijd hetzelfde — en toch als veld,
            // want de bekendmaking legt op dezelfde zaak een gram met een andere
            // stage, en een reductie moet de twee uit elkaar kunnen houden.
            (STAGE.to_string(), Value::String(STAGE_BESLUIT.to_string())),
            (
                CHRONICLE_SOURCES.to_string(),
                Value::Array(
                    self.chronicle_sources
                        .iter()
                        .map(ChronicleSource::as_value)
                        .collect(),
                ),
            ),
            (RECEIPT.to_string(), self.receipt_value()?),
        ]);
        fields.extend(
            self.outputs
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );

        Ok(ChronicleEvent {
            name: self.besluit.clone(),
            intake: Intake::EigenBesluit,
            recording_actor: self.cell.clone(),
            grondslag: self.grondslag(),
            op_moment: self.op_moment,
            fields,
        })
    }

    /// Het receipt als vastlegbare waarde.
    ///
    /// Via serde en niet met de hand overgeschreven: een handmatige kopie zou
    /// bij elke uitbreiding van RFC-013 stil achterlopen, en dan zou het
    /// decretogram niet meer het receipt zijn maar een selectie eruit.
    fn receipt_value(&self) -> Result<Value> {
        let encoded = serde_yaml_ng::to_string(&self.receipt).map_err(|source| {
            SimulatorError::ReceiptEncoding {
                cell: self.cell.clone(),
                besluit: self.besluit.clone(),
                source,
            }
        })?;
        serde_yaml_ng::from_str(&encoded).map_err(|source| SimulatorError::ReceiptEncoding {
            cell: self.cell.clone(),
            besluit: self.besluit.clone(),
            source,
        })
    }

    /// De grondslag van deze vastlegging: de regeling die uitgevoerd is, met de
    /// versie die toen gold.
    fn grondslag(&self) -> String {
        match &self.regulation_valid_from {
            Some(valid_from) => format!("{} (versie {valid_from})", self.regulation),
            None => self.regulation.clone(),
        }
    }
}

/// Waar één uitkomst van een stage vandaan komt: het artikel dat als hook vuurde.
///
/// De herkomst per waarde, zoals een decretogram die per input draagt. Wie leest
/// dat een bezwaartermijn op een bepaalde dag eindigt, hoort te kunnen zien welk
/// artikel van welke wet dat zegt — anders is de uitkomst van de Awb niet te
/// onderscheiden van iets wat de uitvoerder zelf verzon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookHerkomst {
    /// De naam van de uitkomst.
    pub veld: String,
    /// De regeling, de versie en het artikel die haar voortbrachten.
    pub lexogram: ObligationOrigin,
    /// Het punt in de uitvoering waarop het artikel vuurde.
    pub hook_point: String,
}

impl HookHerkomst {
    /// De herkomst als vastlegbare waarde, voor in het gram.
    fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            ("veld".to_string(), Value::String(self.veld.clone())),
            (LEXOGRAM.to_string(), self.lexogram.as_value()),
            (
                "hook_point".to_string(),
                Value::String(self.hook_point.clone()),
            ),
        ]))
    }

    /// Leesbare herkomst voor een verslag.
    pub fn describe(&self) -> String {
        format!("{} uit {}", self.veld, self.lexogram.describe())
    }
}

/// Waar één uitkomst van de **eigen regeling** bij een stage vandaan komt.
///
/// De tegenhanger van [`HookHerkomst`] voor `stage_uitkomsten`: geen hook-punt,
/// want het artikel vuurde niet op de beschikking — het hoort bij de regeling
/// van het besluit zelf, en zegt dat deze uitkomst pas bij deze stage vaststaat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageUitkomstHerkomst {
    /// De naam van de uitkomst.
    pub veld: String,
    /// De regeling, de versie en het artikel die haar voortbrachten.
    pub lexogram: ObligationOrigin,
}

impl StageUitkomstHerkomst {
    /// De herkomst als vastlegbare waarde, voor in het gram.
    fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            ("veld".to_string(), Value::String(self.veld.clone())),
            (LEXOGRAM.to_string(), self.lexogram.as_value()),
        ]))
    }

    /// Leesbare herkomst voor een verslag.
    pub fn describe(&self) -> String {
        format!("{} uit {}", self.veld, self.lexogram.describe())
    }
}

/// Een hook die vuurde maar niet draaide, omdat een input die hij nodig heeft er
/// bij dit besluit niet is.
///
/// Een hook biedt zich aan op rechtskarakter, besluittype en stage, en weet niet
/// welke feiten het besluit waarop hij vuurt draagt. Zou hij het besluit laten
/// omvallen, dan kon een algemene wet elke beschikking van een soort breken
/// omdat één artikel ervan een feit leest dat alleen sommige besluiten hebben.
/// Het besluit gaat dus door, en het gram zegt welk artikel níet toegepast is en
/// waarom — een vereiste zonder toets hoort zichtbaar te blijven.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookNietUitgevoerd {
    /// De regeling, de versie en het artikel van de hook.
    pub lexogram: ObligationOrigin,
    /// Het punt in de uitvoering waarop hij vuurde.
    pub hook_point: String,
    /// De input die er niet was.
    pub ontbrekende_input: String,
}

impl HookNietUitgevoerd {
    /// Als vastlegbare waarde, voor in het gram.
    fn as_value(&self) -> Value {
        Value::Object(BTreeMap::from([
            (LEXOGRAM.to_string(), self.lexogram.as_value()),
            (
                "hook_point".to_string(),
                Value::String(self.hook_point.clone()),
            ),
            (
                "ontbrekende_input".to_string(),
                Value::String(self.ontbrekende_input.clone()),
            ),
        ]))
    }

    /// Leesbare regel voor een verslag en voor het journaal.
    pub fn describe(&self) -> String {
        format!(
            "{} niet uitgevoerd: input '{}' bestaat bij dit besluit niet",
            self.lexogram.describe(),
            self.ontbrekende_input
        )
    }
}

/// De inputs van een gram, elk met hun herkomst, als vastlegbare waarde.
///
/// Eén vorm voor het besluit en de bekendmaking: [`recorded_input`] leest haar
/// terug, en het beeld van de wereld pakt haar uit per waarde.
fn inputs_value(inputs: &BTreeMap<String, DecretogramInput>) -> Value {
    Value::Object(
        inputs
            .iter()
            .map(|(name, input)| {
                (
                    name.clone(),
                    Value::Object(BTreeMap::from([
                        ("value".to_string(), input.value.clone()),
                        ("origin".to_string(), input.origin.as_value()),
                    ])),
                )
            })
            .collect(),
    )
}

/// De hooks die niet draaiden, als vastlegbare waarde.
fn hooks_niet_uitgevoerd_value(hooks: &[HookNietUitgevoerd]) -> Value {
    Value::Array(hooks.iter().map(HookNietUitgevoerd::as_value).collect())
}

/// De **bekendmaking** van een besluit: het stage-decretogram van RFC-008.
///
/// Elke stage van een procedure levert een eigen elementair gram op hetzelfde
/// zaakkenmerk (RFC-022 §1.2), en dit is het tweede: hetzelfde besluit, dezelfde
/// zaak, een andere stage. Het is met opzet géén tweede besluit — er wordt niets
/// opnieuw vastgesteld en de uitkomsten van het besluit staan er niet nog eens in
/// — en evenmin een veld dat aan het eerste gram toegevoegd wordt: een kroniek
/// groeit en wijzigt nooit.
///
/// Wat het draagt, komt van de **wet**: de hooks die op de stage BEKENDMAKING
/// vuren (Awb 3:45, 4:87, 6:8 in de testregeling hier). De uitvoerende regeling
/// weet er niets van, en dat is de winst — de uiterste betaaldatum staat niet in
/// elke uitvoeringsregeling overgeschreven.
#[derive(Debug, Clone)]
pub struct Bekendmaking {
    /// De cel die bekendmaakte; dezelfde die besloot.
    pub cell: String,
    /// Wat er bij de bekendmaking ingevuld is: de `requires` van de stage uit de
    /// procedure, elk als parameter.
    pub inputs: BTreeMap<String, DecretogramInput>,
    /// Waar elke uitkomst van de eigen regeling bij deze stage vandaan komt.
    pub stage_uitkomsten: Vec<StageUitkomstHerkomst>,
    /// De hooks die op deze stage vuurden maar niet konden draaien.
    pub hooks_niet_uitgevoerd: Vec<HookNietUitgevoerd>,
    /// De besluit-definitie waarvan dit de bekendmaking is.
    pub besluit: String,
    /// De zaak waar het om gaat; hetzelfde kenmerk als het besluit.
    pub zaakkenmerk: String,
    /// De dag van de bekendmaking: de stand van de klok, en de waarde van het
    /// datumveld dat de procedure voor deze stage vraagt.
    pub op_moment: NaiveDate,
    /// Het moment van het besluit dat bekendgemaakt wordt.
    pub besluit_op_moment: NaiveDate,
    /// De plek van dat besluit in [`BESCHIKKINGEN`], geteld vanaf nul.
    pub besluit_gram: usize,
    /// De regeling waarvan de stage uitgevoerd is.
    pub regulation: String,
    /// De `valid_from` van de versie die gold bij het besluit.
    pub regulation_valid_from: Option<String>,
    /// Het bevoegd gezag van het besluit (RFC-002).
    pub competent_authority: Option<String>,
    /// De identiteit van de cel die bekendmaakte.
    pub bekendgemaakt_door: String,
    /// Het rechtskarakter: altijd [`BESCHIKKING`], want dat is wat er
    /// bekendgemaakt wordt.
    pub legal_character: String,
    /// Het besluittype van het besluit dat bekendgemaakt wordt.
    pub decision_type: Option<String>,
    /// Wat de hooks en de eigen regeling op deze stage opleverden.
    pub outputs: BTreeMap<String, Value>,
    /// Waar elke uitkomst van een hook vandaan komt.
    pub hooks: Vec<HookHerkomst>,
    /// De termijnen die door deze bekendmaking gaan lopen.
    ///
    /// Leeg als het besluit niets op de bekendmaking liet wachten, en ook als er
    /// wél iets wachtte maar dit besluit inmiddels vervangen is — dan staat in
    /// [`Self::termijnen_vervallen_door`] waardoor.
    pub obligations: Vec<ObligationDue>,
    /// Het besluit dat in de plaats van dit besluit kwam voordat het
    /// bekendgemaakt werd, als dat er is.
    ///
    /// `None` is het gewone geval: er is niets vervangen, en wat er wachtte gaat
    /// gewoon lopen. Staat er wél iets, dan is [`Self::obligations`] leeg — zie
    /// [`TermijnenVervallen`].
    pub termijnen_vervallen_door: Option<TermijnenVervallen>,
    /// Het receipt van de stage-uitvoering.
    pub receipt: ExecutionReceipt,
}

impl Bekendmaking {
    /// De bekendmaking als kroniekgebeurtenis.
    ///
    /// Met hetzelfde kanaal als een decretogram ([`Intake::EigenBesluit`]): het
    /// is iets wat de cel zélf deed en niet iets dat haar overkwam, en het ligt
    /// in dezelfde stroom als het besluit waar het bij hoort. Het veld [`STAGE`]
    /// houdt de twee uit elkaar.
    pub(crate) fn event(&self) -> Result<ChronicleEvent> {
        let mut fields: BTreeMap<String, Value> = BTreeMap::from([
            (
                ZAAKKENMERK.to_string(),
                Value::String(self.zaakkenmerk.clone()),
            ),
            (BESLUIT.to_string(), Value::String(self.besluit.clone())),
            (
                STAGE.to_string(),
                Value::String(STAGE_BEKENDMAKING.to_string()),
            ),
            (INPUTS.to_string(), inputs_value(&self.inputs)),
            (
                BESLUIT_OP_MOMENT.to_string(),
                Value::String(self.besluit_op_moment.to_string()),
            ),
            (
                BESLUIT_GRAM.to_string(),
                Value::Int(i64::try_from(self.besluit_gram).unwrap_or(i64::MAX)),
            ),
            (
                REGULATION.to_string(),
                Value::String(self.regulation.clone()),
            ),
            (
                REGULATION_VALID_FROM.to_string(),
                optional_text(self.regulation_valid_from.as_deref()),
            ),
            (
                COMPETENT_AUTHORITY.to_string(),
                optional_text(self.competent_authority.as_deref()),
            ),
            (
                BEKENDGEMAAKT_DOOR.to_string(),
                Value::String(self.bekendgemaakt_door.clone()),
            ),
            (
                LEGAL_CHARACTER.to_string(),
                Value::String(self.legal_character.clone()),
            ),
            (
                DECISION_TYPE.to_string(),
                optional_text(self.decision_type.as_deref()),
            ),
            (
                HOOKS.to_string(),
                Value::Array(self.hooks.iter().map(HookHerkomst::as_value).collect()),
            ),
            (
                STAGE_UITKOMSTEN.to_string(),
                Value::Array(
                    self.stage_uitkomsten
                        .iter()
                        .map(StageUitkomstHerkomst::as_value)
                        .collect(),
                ),
            ),
            (
                HOOK_NIET_UITGEVOERD.to_string(),
                hooks_niet_uitgevoerd_value(&self.hooks_niet_uitgevoerd),
            ),
            (
                OBLIGATIONS.to_string(),
                Value::Array(
                    self.obligations
                        .iter()
                        .map(ObligationDue::as_value)
                        .collect(),
                ),
            ),
            (
                TERMIJNEN_VERVALLEN_DOOR.to_string(),
                self.termijnen_vervallen_door
                    .as_ref()
                    .map_or(Value::Null, TermijnenVervallen::as_value),
            ),
            (RECEIPT.to_string(), self.receipt_value()?),
        ]);
        fields.extend(
            self.outputs
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );

        Ok(ChronicleEvent {
            name: format!("{}_{}", self.besluit, STAGE_BEKENDMAKING.to_lowercase()),
            intake: Intake::EigenBesluit,
            recording_actor: self.cell.clone(),
            grondslag: format!(
                "bekendmaking van besluit '{}' van {} ({})",
                self.besluit,
                self.besluit_op_moment,
                self.hooks
                    .iter()
                    .map(HookHerkomst::describe)
                    .chain(
                        self.stage_uitkomsten
                            .iter()
                            .map(StageUitkomstHerkomst::describe)
                    )
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            op_moment: self.op_moment,
            fields,
        })
    }

    /// Het receipt als vastlegbare waarde; zie [`Decretogram::receipt_value`].
    fn receipt_value(&self) -> Result<Value> {
        let encoded = serde_yaml_ng::to_string(&self.receipt).map_err(|source| {
            SimulatorError::ReceiptEncoding {
                cell: self.cell.clone(),
                besluit: self.besluit.clone(),
                source,
            }
        })?;
        serde_yaml_ng::from_str(&encoded).map_err(|source| SimulatorError::ReceiptEncoding {
            cell: self.cell.clone(),
            besluit: self.besluit.clone(),
            source,
        })
    }

    /// Leesbare regel voor een verslag: wat deze bekendmaking opleverde.
    pub fn describe(&self) -> String {
        let velden = self
            .outputs
            .iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "bekendmaking van '{}' over zaak '{}' op {} ({velden})",
            self.besluit, self.zaakkenmerk, self.op_moment
        )
    }
}

/// De waarde van één input uit een gram, of `None` als het gram haar niet draagt.
///
/// De tegenhanger van wat [`Decretogram::event`] hierboven schrijft, en daarom
/// hier: de vorm van dat ene veld — `inputs.<naam>.value` — hoort op één plek te
/// staan. Hoofdletterongevoelig, net als elders bij veldnamen.
pub(crate) fn recorded_input<'a>(
    fields: &'a BTreeMap<String, Value>,
    name: &str,
) -> Option<&'a Value> {
    let Some(Value::Object(inputs)) = fields.get(INPUTS) else {
        return None;
    };
    let (_, entry) = inputs
        .iter()
        .find(|(input, _)| input.eq_ignore_ascii_case(name))?;
    let Value::Object(parts) = entry else {
        return None;
    };
    parts.get("value")
}

/// Elke input van een besluit, op naam, zoals het gram ze draagt.
///
/// De meervoudige vorm van [`recorded_input`], en om dezelfde reden hier: dit is
/// wat een **volgende stage** van hetzelfde besluit als parameters meekrijgt. Dat
/// die waarden uit het gram komen en niet opnieuw opgehaald worden, is het punt —
/// een bekendmaking stelt niets opnieuw vast, ook geen feit van een ander.
pub(crate) fn recorded_inputs(fields: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    let Some(Value::Object(inputs)) = fields.get(INPUTS) else {
        return BTreeMap::new();
    };
    inputs
        .iter()
        .filter_map(|(name, entry)| {
            let Value::Object(parts) = entry else {
                return None;
            };
            Some((name.clone(), parts.get("value")?.clone()))
        })
        .collect()
}

/// De verplichtingen die in dit gram op de bekendmaking wachten.
///
/// `None` zodra het veld er wél is maar niet te lezen: dan draagt het gram een
/// belofte waarvan niet vaststaat wat ze inhoudt, en daar valt geen termijn uit
/// te maken. Een gram **zonder** dat veld levert een lege lijst — dat is een
/// besluit dat niets liet wachten, en geen leesfout.
pub(crate) fn wachtende_verplichtingen(
    fields: &BTreeMap<String, Value>,
) -> Option<Vec<WachtendeVerplichting>> {
    match fields.get(WACHT_OP_BEKENDMAKING) {
        None => Some(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(WachtendeVerplichting::from_value)
            .collect(),
        Some(_) => None,
    }
}

/// Wat een gram van één besluit draagt, in de twee lagen waarin het dat doet.
///
/// Los van elkaar en niet op één hoop, want ze liggen in het gram ook niet op
/// één hoop: de uitkomsten en de vaste velden staan er bovenaan, de inputs een
/// laag dieper onder [`INPUTS`]. Een naam die in beide lagen voorkomt zou
/// daarmee twee waarden aanwijzen, en dan is het niet aan de lezer van
/// `from_decretogram: … + field: …` om te raden welke gepakt wordt — zie
/// [`SimulatorError::AmbiguousEarlierBesluitField`].
#[derive(Debug, Clone, Default)]
pub(crate) struct GramFields {
    /// De velden van het gram zelf: de uitkomsten waarop besloten is, plus de
    /// vaste velden van elk decretogram.
    pub(crate) own: BTreeSet<String>,
    /// De inputs waarop dat besluit rekende, elk met hun eigen herkomst.
    pub(crate) inputs: BTreeSet<String>,
}

impl GramFields {
    /// In welke laag ligt dit veld? `None` als het gram het niet draagt.
    ///
    /// Hoofdletterongevoelig, net als elders bij veldnamen — en aan beide kanten
    /// dezelfde toets als bij het lezen zelf, zodat het optuigen niets doorlaat
    /// wat later toch niet gevonden wordt.
    fn layers_with(&self, field: &str) -> (bool, bool) {
        let has =
            |names: &BTreeSet<String>| names.iter().any(|known| known.eq_ignore_ascii_case(field));
        (has(&self.own), has(&self.inputs))
    }

    /// Alle namen die zo'n gram draagt, voor in een foutmelding.
    fn listing(&self) -> String {
        self.own
            .iter()
            .chain(self.inputs.iter())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Een tekst die er kan zijn, als vastlegbare waarde.
///
/// `null` en niet "de lege tekst": dat de regeling geen bevoegd gezag noemt is
/// iets anders dan een bevoegd gezag zonder naam.
fn optional_text(text: Option<&str>) -> Value {
    text.map_or(Value::Null, |value| Value::String(value.to_string()))
}

impl BesluitDefinition {
    /// De uitkomsten die dit besluit vastlegt.
    pub fn recorded_outputs(&self) -> BTreeSet<&str> {
        published_outputs(Some(self.output.as_str()), &self.outputs)
    }

    /// De namen die een gram van dít besluit draagt, en die een volgend besluit
    /// er dus met `from_decretogram` uit kan lezen.
    ///
    /// Bekend vóór het eerste besluit, en dat moet ook: een verwijzing die
    /// hierbuiten valt hoort bij het optuigen te sneuvelen en niet pas op het
    /// moment dat er teruggelezen wordt.
    pub(crate) fn gram_fields(&self) -> GramFields {
        GramFields {
            own: self
                .recorded_outputs()
                .into_iter()
                .map(str::to_string)
                // De velden van het **besluit** en niet die van zijn
                // bekendmaking: `from_decretogram` leest het besluit-gram terug
                // (zie [`crate::Cell::read_earlier_decretogram`]), en wat in een
                // ander gram staat, staat daar niet in.
                .chain(FIXED_FIELDS.iter().map(|field| (*field).to_string()))
                .collect(),
            inputs: self.inputs.keys().cloned().collect(),
        }
    }

    /// Controleer de meegegeven parameters tegen de gedocumenteerde.
    pub(crate) fn check_params(&self, cell: &str, params: &BTreeMap<String, Value>) -> Result<()> {
        check_documented_params(cell, Subject::Besluit, &self.name, &self.params, params)
    }

    /// Het zaakkenmerk voor deze parameters.
    ///
    /// Aanroepen ná [`Self::check_params`]: elke verwijzing is bij het optuigen
    /// aan een gedocumenteerde parameter gebonden, en die is dan aanwezig.
    ///
    /// Weigert een waarde waarin een scheidingsteken van het sjabloon zelf
    /// voorkomt. Zie [`Template::check_separators_absent`]: zonder die weigering
    /// zouden twee verschillende zaken hetzelfde kenmerk kunnen krijgen, en dan
    /// levert een reductie op dat kenmerk het besluit van een ander.
    pub(crate) fn zaakkenmerk(
        &self,
        cell: &str,
        params: &BTreeMap<String, Value>,
    ) -> Result<String> {
        let template = Template::parse(&self.zaakkenmerk);
        template.check_separators_absent(cell, &self.name, &self.zaakkenmerk, params)?;
        Ok(template.fill(params))
    }

    /// De parameter waarmee het zaakkenmerk van dit besluit de zaak identificeert.
    ///
    /// Precies één **verschillende** verwijzing, of niets. `zorgtoeslag/{bsn}`
    /// wijst een persoon aan en `{bsn}/{bsn}` nog steeds dezelfde;
    /// `{jaar}/{bsn}` wijst een zaak aan maar geen partij, en dan valt er niet te
    /// kiezen zonder te raden wie van de twee de schuldeiser is.
    fn identifying_param(&self) -> Option<&str> {
        let template = Template::parse(&self.zaakkenmerk);
        let distinct: BTreeSet<&str> = template
            .parts
            .iter()
            .map(|part| part.reference)
            .collect::<BTreeSet<_>>();
        match distinct.len() {
            1 => distinct.into_iter().next(),
            _ => None,
        }
    }

    /// De parameters waarnaar het zaakkenmerk-sjabloon verwijst en die deze
    /// vraag niet noemt, in de volgorde van het sjabloon.
    ///
    /// Bij het besluit zelf is dit altijd leeg — `check_params` eist elke
    /// gedocumenteerde parameter — maar bij de droogloop van
    /// [`crate::Cell::missing_own_fact`] niet: daar staat het formulier met zijn
    /// voorinvulling, en een voorinvulling die nergens op uitkomt levert geen
    /// waarde. [`Self::zaakkenmerk`] vult zo'n verwijzing met niets, en dan zou
    /// er in een melding een lege zaak staan waar een lezer een kenmerk
    /// verwacht. Wie dit eerst vraagt, kan zeggen wat er nog ontbreekt.
    pub(crate) fn zaakkenmerk_gaps(&self, params: &BTreeMap<String, Value>) -> Vec<String> {
        Template::parse(&self.zaakkenmerk)
            .references()
            .filter(|reference| !params.contains_key(*reference))
            .map(str::to_string)
            .collect()
    }

    /// Reken de verplichtingen van dit besluit uit tot termijnen.
    ///
    /// Aanroepen ná de uitvoering: het bedrag komt uit de uitkomsten waarop
    /// besloten is, en niet uit de configuratie. Wat hier ontstaat is het
    /// volledige schema — alle vervaldata, alle bedragen, alle volgnummers —
    /// zodat het in het gram kan en niemand later hoeft te raden wat er beloofd
    /// was.
    ///
    /// `declared` komt uit het **lexogram**: het artikel dat de sturende
    /// uitkomst voortbrengt, in de versie die op `op_moment` gold. Wie er
    /// schuldenaar en schuldeiser is, staat daar ook — het is de
    /// rechtsverhouding die het besluit schept. Welke **cel** die schuldenaar in
    /// deze wereld is, komt uit het **wereldbestand** (`scope.parties`). Die twee
    /// bronnen zijn de hele scheiding waar dit pad om draait: wat er moet
    /// gebeuren staat in de wet, wie het doet in de uitvoering.
    ///
    /// Wat eruit komt valt in tweeën. De termijnen die nú vervallen — met hun
    /// vervaldatum, want die staat vast — en de verplichtingen die op de
    /// **bekendmaking** wachten (`vanaf: bekendmaking`). Die laatste dragen alles
    /// behalve een dag: een besluit dat nog niet bekendgemaakt is, werkt niet en
    /// er valt dus niets in te roosteren (Awb 3:40). Ze komen als
    /// [`WachtendeVerplichting`] mee in het gram, en de bekendmaking maakt er
    /// termijnen van.
    pub(crate) fn schedule_obligations(
        &self,
        scope: ObligationScope<'_>,
        declared: &DeclaredObligations,
        outputs: &BTreeMap<String, Value>,
        params: &BTreeMap<String, Value>,
        settings: &BTreeMap<String, Value>,
    ) -> Result<Verplichtingenschema> {
        let ObligationScope {
            cell,
            parties,
            zaakkenmerk,
            op_moment,
        } = scope;
        // Eerst elke verplichting oplossen, dan pas termijnen maken: het aantal
        // termijnen van het hele schema staat in elke termijn, en dat is pas
        // bekend als alle ritmes eruit zijn.
        let mut resolved = Vec::with_capacity(declared.items.len());
        let mut niets_te_betalen = Vec::new();
        for obligation in &declared.items {
            let schedule = obligation.schedule(cell, &self.name, settings)?;
            let start = obligation.start(cell, &self.name, params, op_moment)?;
            let total = obligation.total(cell, &self.name, outputs)?;
            // Ná het bedrag, want het teken bepaalt de richting: een negatief
            // slotbedrag is geen negatieve betaling maar een verplichting de
            // andere kant op.
            let relation = obligation.relation(cell, self, declared, params, total)?;
            // Precies nul is niets te betalen, en dat is geen termijn van nul:
            // een executogram van nul euro legt iets vast dat niet gebeurde. Ze
            // telt ook niet mee in het schema — geen volgnummer, en niet in "van
            // zoveel" — maar ze staat wél in het gram (zie [`NIETS_TE_BETALEN`]).
            if relation.total.is_zero() {
                niets_te_betalen.push(NietsTeBetalen {
                    soort: relation.soort,
                    schuldenaar: relation.schuldenaar,
                    schuldeiser: relation.schuldeiser,
                    schedule,
                    grondslag: obligation.grondslag.clone(),
                    herkomst: declared.origin.clone(),
                });
                continue;
            }
            resolved.push((obligation, schedule, start, relation));
        }
        // De termijnen van dit besluit tellen nooit zo hoog dat ze de grenzen van
        // i64 raken: het zijn er ten hoogste twaalf per verplichting.
        let termijnen = i64::try_from(
            resolved
                .iter()
                .map(|(_, schedule, _, _)| schedule.terms() as usize)
                .sum::<usize>(),
        )
        .unwrap_or(i64::MAX);

        let mut due = Vec::new();
        let mut wachtend = Vec::new();
        // De volgnummers lopen door over álle verplichtingen van dit besluit, ook
        // over de wachtende heen: wat er straks bij de bekendmaking bij komt,
        // hoort niet opnieuw bij 1 te beginnen. Daarom telt deze teller ook de
        // termijnen die hier nog geen dag krijgen.
        let mut volgnummer: i64 = 0;
        for (obligation, schedule, start, relation) in resolved {
            // Wie er in déze wereld onder de naam van de schuldenaar nakomt. Geen
            // cel is geen fout: de wereld kent die actor niet, de termijn staat
            // open (zie [`ObligationDue::betaler`]).
            let betaler = parties.cell_for(&relation.schuldenaar).map(str::to_string);
            let start = match start {
                ObligationStart::Op(day) => day,
                // Wacht op de bekendmaking: alles ligt vast behalve de dag.
                ObligationStart::Bekendmaking => {
                    // Onbereikbaar leeg: het optuigen weigert `vanaf:
                    // bekendmaking` zonder `vervaldatum`, voor elke versie.
                    let vervaldatum_uit = obligation.vervaldatum.clone().ok_or_else(|| {
                        SimulatorError::ObligationVervaldatum {
                            cell: cell.to_string(),
                            besluit: self.name.clone(),
                            origin: declared.origin.describe(),
                            reason: format!("`vanaf: {VANAF_BEKENDMAKING}` zonder `vervaldatum`"),
                        }
                    })?;
                    wachtend.push(WachtendeVerplichting {
                        vervaldatum_uit,
                        soort: relation.soort,
                        schuldenaar: relation.schuldenaar.clone(),
                        schuldeiser: relation.schuldeiser.clone(),
                        betaler: betaler.clone(),
                        bedrag: relation.total,
                        schedule,
                        eerste_volgnummer: volgnummer + 1,
                        termijnen,
                        grondslag: obligation.grondslag.clone(),
                        herkomst: declared.origin.clone(),
                    });
                    volgnummer += i64::from(schedule.terms());
                    continue;
                }
            };
            for (index, bedrag) in split(relation.total, schedule.terms())
                .into_iter()
                .enumerate()
            {
                // `index` telt de termijnen van dit ritme en komt nooit in de
                // buurt van de grens van u32.
                let step = u32::try_from(index).unwrap_or(u32::MAX);
                let vervaldatum = start
                    .checked_add_months(Months::new(step * schedule.step_months()))
                    .ok_or_else(|| SimulatorError::MalformedObligationDate {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        template: obligation.vanaf.clone().unwrap_or_else(|| start.to_string()),
                        reason: format!(
                            "de termijn {} maanden na {start} valt buiten het bereik van de kalender",
                            step * schedule.step_months()
                        ),
                    })?;
                // Vóór het besluit kan er niet betaald worden: wat er dan al
                // vervallen had moeten zijn, wordt vandaag ingehaald.
                let (vervaldatum, oorspronkelijke_vervaldatum) = inhalen(vervaldatum, op_moment);

                volgnummer += 1;
                due.push(ObligationDue {
                    soort: relation.soort,
                    schuldenaar: relation.schuldenaar.clone(),
                    schuldeiser: relation.schuldeiser.clone(),
                    betaler: betaler.clone(),
                    decided_by: cell.to_string(),
                    besluit: self.name.clone(),
                    zaakkenmerk: zaakkenmerk.to_string(),
                    decided_op_moment: op_moment,
                    schedule,
                    vervaldatum,
                    oorspronkelijke_vervaldatum,
                    bedrag: amount(bedrag),
                    grondslag: obligation.grondslag.clone(),
                    herkomst: declared.origin.clone(),
                    // Door het hele schema heen, niet per verplichting opnieuw:
                    // zie [`ObligationDue::volgnummer`].
                    volgnummer,
                    termijnen,
                    // De plek van het gram is hier nog niet bekend: het besluit
                    // is nog niet vastgelegd, en deze termijnen gaan er juist in
                    // mee. `Cell::decide` vult hem in zodra het gram ligt.
                    besluit_gram: 0,
                });
            }
        }
        Ok(Verplichtingenschema {
            termijnen: due,
            wachtend,
            niets_te_betalen,
        })
    }

    /// Controleer de definitie tegen de cel waarin ze staat.
    ///
    /// Een besluit belooft dat het uit te voeren is: op een eigen regeling, met
    /// uitkomsten die die regeling kent, met inputs die ze declareert, uit
    /// stromen die de cel houdt. Dat blijkt hier — bij het optuigen — en niet
    /// pas op het moment dat er besloten moet worden.
    pub(crate) fn validate(&self, cell: &str, surface: &CellSurface<'_>) -> Result<()> {
        surface.check_own_regulation(cell, Subject::Besluit, &self.name, &self.regulation)?;
        surface.check_regulation_outputs(
            cell,
            Subject::Besluit,
            &self.name,
            &self.regulation,
            self.recorded_outputs(),
        )?;
        // De aansturende uitkomst *is* het besluit, dus zij moet een beschikking
        // zijn — onder elke geladen versie, want een besluit over een ouder
        // moment landt op een oudere versie (RFC-022 §1.2).
        surface.check_beschikking(cell, &self.name, &self.regulation, &self.output)?;
        self.reject_afwijzing_wanneer(cell)?;
        self.validate_afwijzing_wanneer(cell, surface)?;

        // De uitkomsten komen in hetzelfde gram als de vaste velden. Een
        // uitkomst die zo heet, zou er een overschrijven — het gram zou dan
        // bijvoorbeeld zijn receipt kwijt zijn zonder dat iemand het merkt.
        let bezet = beschikkingen_fields();
        for output in self.recorded_outputs() {
            if bezet.contains(&output) {
                return Err(SimulatorError::ReservedDecretogramField {
                    cell: cell.to_string(),
                    besluit: self.name.clone(),
                    output: output.to_string(),
                    fixed: bezet.into_iter().collect::<Vec<_>>().join(", "),
                });
            }
        }

        let known_inputs = surface
            .regulation_inputs
            .get(&self.regulation)
            .cloned()
            .unwrap_or_default();
        for (input, origin) in &self.inputs {
            if !known_inputs
                .iter()
                .any(|known| known.eq_ignore_ascii_case(input))
            {
                return Err(SimulatorError::UnknownRegulationInput {
                    cell: cell.to_string(),
                    besluit: self.name.clone(),
                    input: input.clone(),
                    regulation: self.regulation.clone(),
                    known: known_inputs
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
            self.validate_input(cell, surface, input, origin)?;
        }

        // Verplichtingen horen in het lexogram. Een wereldbestand dat ze nog
        // zelf opschrijft, zou zeggen dat het beleid van deze uitvoerder is wat
        // de wet voorschrijft — en twee uitvoerders van dezelfde regeling zouden
        // dan een ander schema kunnen krijgen.
        if self.obligations == ObsoleteField::Present {
            return Err(SimulatorError::ObligationsInWorldFile {
                cell: cell.to_string(),
                besluit: self.name.clone(),
                regulation: self.regulation.clone(),
                output: self.output.clone(),
            });
        }
        for declared in surface.obligations_of(&self.regulation, &self.output) {
            declared.validate(cell, self)?;
        }

        self.validate_zaakkenmerk(cell)
    }

    /// De afwijzingsvoorwaarden horen in de regeling, niet hier.
    ///
    /// Wanneer een besluit een afwijzing is, hangt aan de uitkomst die het
    /// artikel voortbrengt en geldt voor elke cel die dat artikel uitvoert. Een
    /// wereldbestand dat het overschrijft, zou twee uitvoerders dezelfde wet
    /// verschillend laten weigeren zonder dat er aan de wet iets te zien is.
    fn reject_afwijzing_wanneer(&self, cell: &str) -> Result<()> {
        if self.afwijzing_wanneer.is_none() {
            return Ok(());
        }
        Err(SimulatorError::AfwijzingWanneerInWereldbestand {
            cell: cell.to_string(),
            besluit: self.name.clone(),
            regulation: self.regulation.clone(),
            output: self.output.clone(),
        })
    }

    /// De afwijzingsvoorwaarden die de regeling voor dít besluit declareert,
    /// getoetst tegen wat ze kent.
    ///
    /// Bij het optuigen en niet bij het eerste besluit, en over **elke** geladen
    /// versie: een besluit over een ouder moment landt op een oudere versie, en
    /// een voorwaarde die daar nergens op slaat hoort niet pas dan te blijken.
    /// Een naam die de regeling niet kent, zou nooit vervuld raken; een uitkomst
    /// die geen ja-of-nee is evenmin — en dan ligt er een toekenning waar een
    /// weigering hoorde.
    fn validate_afwijzing_wanneer(&self, cell: &str, surface: &CellSurface<'_>) -> Result<()> {
        for block in surface.afwijzing_blocks(&self.regulation, &self.output) {
            let conditions = afwijzing_wanneer(block).map_err(|reason| {
                SimulatorError::MalformedAfwijzingWanneer {
                    cell: cell.to_string(),
                    besluit: self.name.clone(),
                    regulation: self.regulation.clone(),
                    output: self.output.clone(),
                    reason,
                }
            })?;
            for output in conditions.keys() {
                surface.check_regulation_outputs(
                    cell,
                    Subject::Besluit,
                    &self.name,
                    &self.regulation,
                    [output.as_str()],
                )?;
                surface.check_boolean_output(cell, &self.name, &self.regulation, output)?;
            }
        }
        Ok(())
    }

    /// Eén input: is de stroom er een om feiten uit te lezen, bestaat ze, kent
    /// ze het veld, en is haar sleutel aan te leveren? Of, bij een parameter: is
    /// die gedocumenteerd?
    fn validate_input(
        &self,
        cell: &str,
        surface: &CellSurface<'_>,
        input: &str,
        origin: &BesluitInput,
    ) -> Result<()> {
        match origin {
            BesluitInput::FromChronicle { chronicle, field } => {
                // Een besluit leest geen besluit. De stroom met decretogrammen is
                // een gewone stroom zodra een cel besluit-definities heeft, dus
                // zonder deze weigering kan een besluit een veld van een eerder
                // decretogram als "eigen feit" binnenhalen — dezelfde
                // schaduwboekhouding die `register_own_facts` aan de kant van de
                // engine al buiten de deur houdt, langs de andere weg.
                if chronicle == BESCHIKKINGEN {
                    return Err(SimulatorError::DecretogramAsBesluitInput {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        input: input.to_string(),
                        stream: chronicle.clone(),
                        field: field.clone(),
                    });
                }
                surface.check_stream_field(cell, Subject::Besluit, &self.name, chronicle, field)?;
                // Onbereikbaar leeg: `check_stream_field` heeft de stroom
                // hierboven al gevonden, en elke stroom declareert een sleutel.
                let key = surface
                    .stream_key(chronicle)
                    .unwrap_or_default()
                    .to_string();
                if !documents(&self.params, &key) {
                    return Err(SimulatorError::BesluitStreamKeyWithoutParameter {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        stream: chronicle.clone(),
                        key,
                        documented: parameter_listing(&self.params),
                    });
                }
                Ok(())
            }
            BesluitInput::Param { param } => {
                if documents(&self.params, param) {
                    return Ok(());
                }
                Err(SimulatorError::UnknownReference {
                    cell: cell.to_string(),
                    subject: Subject::Besluit,
                    name: self.name.clone(),
                    reference: param.clone(),
                })
            }
            BesluitInput::FromDecretogram {
                besluit: earlier,
                field,
            } => {
                // De cel moet het besluit kennen waaruit ze terugleest. Een
                // typfout hier zou bij elk besluit "geen eerder besluit"
                // opleveren, en dat is niet te onderscheiden van een zaak die
                // nog geen geschiedenis heeft.
                let Some(fields) = surface.besluit_fields.get(earlier) else {
                    return Err(SimulatorError::UnknownEarlierBesluit {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        earlier: earlier.clone(),
                        known: surface
                            .besluit_fields
                            .keys()
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                            .join(", "),
                    });
                };
                // Eén naam kan niet twee waarden aanwijzen. Draagt zo'n gram het
                // veld in beide lagen — als uitkomst of vast veld én als input —
                // dan is er geen volgorde te kiezen die niet af en toe het
                // verkeerde getal oplevert, en het beeld van de wereld kiest hier
                // al andersom dan een leesregel hier zou doen (zie
                // `snapshot::gram_snapshot`). Dus bij het optuigen weigeren in
                // plaats van stil een winnaar aanwijzen.
                match fields.layers_with(field) {
                    (true, true) => Err(SimulatorError::AmbiguousEarlierBesluitField {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        earlier: earlier.clone(),
                        field: field.clone(),
                    }),
                    (false, false) => Err(SimulatorError::UnknownEarlierBesluitField {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        earlier: earlier.clone(),
                        field: field.clone(),
                        known: fields.listing(),
                    }),
                    _ => Ok(()),
                }
            }
            BesluitInput::AcceptFrom {
                cell: peer, params, ..
            } => {
                // De eigen cel is geen peer. Voor eigen feiten is er een
                // kroniek of een eigen wet; wie zichzelf over de grens
                // bevraagt, zet een cross-cel-contact in het vraaggraf dat er
                // niet hoort en zou bij de veiligheidscontext alsnog stuklopen
                // — beter hier, bij het optuigen.
                if peer == cell {
                    return Err(SimulatorError::AcceptFromSelf {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        input: input.to_string(),
                    });
                }
                for binding in params.values() {
                    // De ingebouwde verwijzing naar het zaakkenmerk van het
                    // lopende besluit is geen parameter en hoeft dus niet
                    // gedocumenteerd te zijn — ze moet alleen iets te betekenen
                    // hebben. Documenteert de definitie tóch een parameter die
                    // zo heet, dan staat er één naam voor twee dingen; dan is de
                    // vraag niet meer te lezen zoals ze er staat.
                    if binding == ZAAKKENMERK_REFERENCE {
                        self.check_zaakkenmerk_reference(cell, input)?;
                        continue;
                    }
                    let Some(reference) = binding_name(binding) else {
                        continue;
                    };
                    if !documents(&self.params, reference) {
                        return Err(SimulatorError::UnknownReference {
                            cell: cell.to_string(),
                            subject: Subject::Besluit,
                            name: self.name.clone(),
                            reference: reference.to_string(),
                        });
                    }
                }
                // Of de peer bestaat en deze naam publiceert, weet de cel niet:
                // ze kent geen andere cel. Dat valt bij de wereld (die de peers
                // kent) en anders bij het transport.
                Ok(())
            }
        }
    }

    /// Mag `$zaakkenmerk` in de vraag van deze input staan?
    ///
    /// Twee dingen moeten kloppen, en allebei bij het optuigen: er moet een
    /// zaakkenmerk-sjabloon zijn om in te vullen, en de naam mag niet ook een
    /// gedocumenteerde parameter zijn. Het sjabloon is vandaag verplicht, dus de
    /// eerste toets vangt de lege vorm; zou het ooit weg mogen blijven, dan blijft
    /// de regel staan in plaats van stil een lege tekst over de grens te sturen.
    fn check_zaakkenmerk_reference(&self, cell: &str, input: &str) -> Result<()> {
        if self.zaakkenmerk.trim().is_empty() {
            return Err(SimulatorError::ZaakkenmerkReferenceWithoutTemplate {
                cell: cell.to_string(),
                besluit: self.name.clone(),
                input: input.to_string(),
            });
        }
        if documents(&self.params, ZAAKKENMERK) {
            return Err(SimulatorError::AmbiguousZaakkenmerkReference {
                cell: cell.to_string(),
                besluit: self.name.clone(),
                input: input.to_string(),
            });
        }
        Ok(())
    }

    /// De waarden die dit besluit bij een andere cel moet ophalen.
    ///
    /// Aanroepen ná [`Self::check_params`]: de verwijzingen in `params` zijn bij
    /// het optuigen aan gedocumenteerde parameters gebonden, en die zijn dan
    /// aanwezig. `zaakkenmerk` is het ingevulde kenmerk van dit besluit, want een
    /// vraag mag ernaar verwijzen ([`ZAAKKENMERK_REFERENCE`]) en de definitie
    /// kent het pas als de parameters erin zitten.
    ///
    /// Leeg is het normale geval: een besluit dat alles zelf weet, vraagt
    /// niemand iets.
    pub(crate) fn acceptance_requests(
        &self,
        params: &BTreeMap<String, Value>,
        zaakkenmerk: &str,
    ) -> Vec<AcceptanceRequest> {
        // Het kenmerk gaat als gewone waarde de invuller in, en overschrijft
        // daarmee een parameter die toevallig zo heet. Dat kan alleen bij een
        // definitie die `$zaakkenmerk` nergens gebruikt — die combinatie is bij
        // het optuigen geweigerd — en het houdt het invullen op één plek.
        let mut params = params.clone();
        params.insert(
            ZAAKKENMERK.to_string(),
            Value::String(zaakkenmerk.to_string()),
        );
        let params = &params;
        self.inputs
            .iter()
            .filter_map(|(input, origin)| match origin {
                BesluitInput::AcceptFrom {
                    cell,
                    lexostatus,
                    field,
                    params: bindings,
                } => Some(AcceptanceRequest {
                    input: input.clone(),
                    cell: cell.clone(),
                    lexostatus: lexostatus.clone(),
                    field: field.clone(),
                    params: engine_parameters(bindings, params),
                }),
                BesluitInput::FromChronicle { .. }
                | BesluitInput::Param { .. }
                | BesluitInput::FromDecretogram { .. } => None,
            })
            .collect()
    }

    /// Het zaakkenmerk-sjabloon: sluitende accolades, minstens één verwijzing,
    /// elke verwijzing een gedocumenteerde parameter, en tussen twee
    /// verwijzingen iets dat ze uit elkaar houdt.
    fn validate_zaakkenmerk(&self, cell: &str) -> Result<()> {
        if !closing_braces_match(&self.zaakkenmerk) {
            return Err(SimulatorError::MalformedZaakkenmerk {
                cell: cell.to_string(),
                besluit: self.name.clone(),
                template: self.zaakkenmerk.clone(),
            });
        }

        let template = Template::parse(&self.zaakkenmerk);
        for part in &template.parts {
            if !documents(&self.params, part.reference) {
                return Err(SimulatorError::UnknownReference {
                    cell: cell.to_string(),
                    subject: Subject::Besluit,
                    name: self.name.clone(),
                    reference: part.reference.to_string(),
                });
            }
        }

        if template.parts.is_empty() {
            return Err(SimulatorError::ZaakkenmerkWithoutReference {
                cell: cell.to_string(),
                besluit: self.name.clone(),
                template: self.zaakkenmerk.clone(),
            });
        }

        // Twee verwijzingen die aan elkaar plakken zijn nooit uit elkaar te
        // houden: `{jaar}{bsn}` met 2024 + 999993653 levert hetzelfde kenmerk
        // als 20249 + 99993653. Geen waarde kan dat repareren, dus dit is een
        // optuigfout en geen weigering bij het besluit.
        for pair in template.parts.windows(2) {
            if pair[0].literal.is_empty() {
                return Err(SimulatorError::AdjacentZaakkenmerkReferences {
                    cell: cell.to_string(),
                    besluit: self.name.clone(),
                    template: self.zaakkenmerk.clone(),
                    first: pair[0].reference.to_string(),
                    second: pair[1].reference.to_string(),
                });
            }
        }

        Ok(())
    }
}

impl DeclaredObligations {
    /// Lees wat één artikel onder
    /// [`CHRONOLEX`](crate::cell::extensions::CHRONOLEX) declareert.
    ///
    /// Een artikel zonder blok legt niets op, en dat is geen fout: niet elke
    /// beschikking kent een bedrag toe. Een blok dat er wél staat maar niet
    /// klopt, is er wel een — het staat in de wet en niet in een wereldbestand,
    /// dus stil overslaan zou een regeling laten zwijgen waar ze spreekt.
    pub(crate) fn from_article(
        origin: ObligationOrigin,
        authority: Option<String>,
        article: &Article,
    ) -> Result<Self> {
        let block = ChronolexBlock::read(
            article
                .get_execution_spec()
                .and_then(|execution| execution.produces.as_ref()),
            &origin,
        )?;
        // Zonder de uitkomsten van de hooks: die dienen alleen de toets op
        // `vervaldatum`, en die heeft het optuigen voor elke versie al gedaan.
        Ok(Self::from_block(
            origin,
            authority,
            article,
            block,
            &BTreeSet::new(),
        ))
    }

    /// Hetzelfde, uit een blok dat al gelezen is.
    ///
    /// Voor de aanroeper die het blok óók voor iets anders nodig heeft (het
    /// optuigen leest er ook de afwijzingsvoorwaarden uit): die hoort het niet
    /// tweemaal te lezen, want twee lezingen van hetzelfde blok kunnen uiteen
    /// gaan lopen.
    pub(crate) fn from_block(
        origin: ObligationOrigin,
        authority: Option<String>,
        article: &Article,
        block: ChronolexBlock,
        hook_outputs: &BTreeSet<String>,
    ) -> Self {
        Self {
            origin,
            authority,
            article_outputs: article
                .get_output_names()
                .into_iter()
                .map(str::to_string)
                .collect(),
            stage_outputs: hook_outputs
                .iter()
                .cloned()
                .chain(
                    block
                        .stage_uitkomsten_voor(STAGE_BEKENDMAKING)
                        .iter()
                        .cloned(),
                )
                .collect(),
            items: block.verplichtingen,
            vervanging: block.vervangt_openstaande_termijnen,
        }
    }

    /// Een regeling die op dit moment niets oplegt.
    ///
    /// Bijvoorbeeld omdat de cel geen engine heeft, of omdat er op dit moment
    /// geen versie geldt. Het besluit valt daar niet op om: dát wordt elders
    /// geweigerd, met een melding die zegt wat er mist.
    pub(crate) fn none(regulation: &str) -> Self {
        Self {
            origin: ObligationOrigin {
                regulation: regulation.to_string(),
                valid_from: None,
                article: String::new(),
            },
            authority: None,
            article_outputs: BTreeSet::new(),
            stage_outputs: BTreeSet::new(),
            items: Vec::new(),
            vervanging: None,
        }
    }

    /// Legt dit artikel iets op?
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// De uitkomsten waarover deze verplichtingen gaan.
    ///
    /// Ze moeten mee de uitvoering in als gevraagde uitkomst: een bedrag mag een
    /// uitkomst van het artikel zijn die het besluit zelf niet publiceert, en dan
    /// rekent de engine haar alleen uit als er om gevraagd wordt.
    pub(crate) fn amounts(&self) -> impl Iterator<Item = &str> {
        self.items
            .iter()
            .map(|obligation| obligation.bedrag.trim_start_matches('$'))
    }

    /// De instellingen van het wereldbestand waarop deze verplichtingen leunen.
    ///
    /// Vandaag is dat alleen het ritme (`ritme: $naam`). Wat het oplevert, is wat
    /// vast komt te staan zodra er op besloten is: het gram legt vast waarop
    /// besloten is, dus een instelling die er daarna onder vandaan geschoven
    /// wordt, laat het gram iets anders zeggen dan er gebeurd is (zie
    /// [`crate::World::update_settings`]).
    pub(crate) fn settings_used(&self) -> BTreeSet<&str> {
        self.items
            .iter()
            .filter_map(|obligation| obligation.ritme.strip_prefix('$'))
            .collect()
    }

    /// Controleer deze verplichtingen tegen de instellingen van de wereld.
    ///
    /// Apart van [`Self::validate`], omdat het antwoord niet in de cel staat: een
    /// `ritme: $betalingsritme` verwijst naar het wereldbestand, en een cel kent
    /// dat niet. De wereld roept dit aan bij het optuigen, zodat een instelling
    /// die niet bestaat of geen ritme is meteen blijkt en niet pas bij het
    /// besluit dat erop leunt.
    pub(crate) fn check_settings(
        &self,
        cell: &str,
        besluit: &str,
        settings: &BTreeMap<String, Value>,
    ) -> Result<()> {
        for obligation in &self.items {
            obligation.schedule(cell, besluit, settings)?;
        }
        Ok(())
    }

    /// Controleer deze verplichtingen tegen het besluit dat het artikel uitvoert.
    pub(crate) fn validate(&self, cell: &str, definition: &BesluitDefinition) -> Result<()> {
        for obligation in &self.items {
            obligation.validate(
                cell,
                definition,
                &self.origin,
                &self.article_outputs,
                &self.stage_outputs,
            )?;
        }
        Ok(())
    }

    /// De partijen die zonder besluit al vaststaan: het bevoegd gezag.
    ///
    /// Voor het **optuigen** van de wereld, die wil weten of de cel die straks
    /// nakomt een betalingsstroom houdt. Een `$parameter` staat er niet bij: die
    /// krijgt pas bij het besluit een waarde, en over een cel die er dan misschien
    /// is valt vooraf niets te toetsen.
    ///
    /// **Beide** kanten worden opgelost en niet alleen de schuldenaar: een
    /// `#bevoegd_gezag` onder een regeling die er geen aanwijst is hier een fout,
    /// ook als die kant de schuldeiser is. Zou alleen de schuldenaar langskomen,
    /// dan viel zo'n verplichting pas bij het eerste besluit om — halverwege de
    /// tijdlijn, in plaats van hier.
    ///
    /// Wat er in de lijst terechtkomt is iets anders dan wat er getoetst wordt:
    /// alleen wie **schuldenaar kan worden** telt mee, want alleen die hoeft een
    /// betalingsstroom te houden. De schuldeiser wordt dat pas als de verplichting
    /// `richting_bij_negatief` declareert.
    pub(crate) fn static_parties(
        &self,
        cell: &str,
        definition: &BesluitDefinition,
    ) -> Result<BTreeSet<String>> {
        let mut names = BTreeSet::new();
        for obligation in &self.items {
            let (schuldenaar, schuldeiser) = obligation.parties(cell, definition, &self.origin)?;
            let keert_om = obligation.richting_bij_negatief.is_some();
            for (role, party, kan_nakomen) in [
                (SCHULDENAAR, schuldenaar, true),
                (SCHULDEISER, schuldeiser, keert_om),
            ] {
                if party == PartyRef::Authority {
                    let name = party.resolve(role, cell, definition, self, &BTreeMap::new())?;
                    if kan_nakomen {
                        names.insert(name);
                    }
                }
            }
        }
        Ok(names)
    }

    /// De soorten nakoming die uit deze verplichtingen kunnen ontstaan.
    ///
    /// Altijd [`ObligationKind::Betaling`] — dat is de enige soort die een
    /// artikel mag declareren — en daarnáást de omgekeerde soort zodra één
    /// verplichting `richting_bij_negatief` draagt: dan kan het bedrag onder nul
    /// uitvallen en is wat er vastgelegd wordt een terugvordering, met een eigen
    /// naam in de kroniek (zie [`ObligationKind::gedaan`]).
    ///
    /// Voor het **optuigen**, om dezelfde reden als waarom het de stroom zelf
    /// vraagt: een stroom waarvan het schema die naam niet kent, zou de eerste
    /// omkering pas op de vervaldatum laten stranden — halverwege de tijdlijn,
    /// in plaats van hier.
    pub(crate) fn soorten(&self) -> Vec<ObligationKind> {
        let mut soorten = vec![ObligationKind::Betaling];
        if self
            .items
            .iter()
            .any(|obligation| obligation.richting_bij_negatief.is_some())
        {
            soorten.push(ObligationKind::Betaling.reversed());
        }
        soorten
    }
}

impl ObligationDefinition {
    /// De twee partijen van deze verplichting, als verwijzingen.
    ///
    /// Eén plek voor het optuigen én het besluit, en dus ook één plek waar de
    /// **standaarden** staan: zou het optuigen een andere standaard invullen dan
    /// het besluit, dan droeg een gram een schuldeiser die bij de toets nooit
    /// langskwam.
    ///
    /// Standaard is de schuldenaar het bevoegd gezag — het gewone geval van een
    /// beschikking die een bedrag toekent — en de schuldeiser de parameter waarmee
    /// het zaakkenmerk de zaak identificeert: een beschikking gaat over iemand, en
    /// dat is de partij die het geld krijgt. Wijst het zaakkenmerk niet precies
    /// één parameter aan, dan valt er niets te raden en moet de wet het zeggen.
    fn parties<'a>(
        &'a self,
        cell: &str,
        definition: &'a BesluitDefinition,
        origin: &ObligationOrigin,
    ) -> Result<(PartyRef<'a>, PartyRef<'a>)> {
        let unreadable = |role: &str, reference: &str| SimulatorError::ObligationParty {
            cell: cell.to_string(),
            besluit: definition.name.clone(),
            origin: origin.describe(),
            role: role.to_string(),
            reason: format!("'{reference}' is geen verwijzing; {PARTY_FORMS}"),
        };
        let schuldenaar = match &self.schuldenaar {
            None => PartyRef::Authority,
            Some(text) => PartyRef::parse(text).ok_or_else(|| unreadable(SCHULDENAAR, text))?,
        };
        let schuldeiser = match &self.schuldeiser {
            Some(text) => PartyRef::parse(text).ok_or_else(|| unreadable(SCHULDEISER, text))?,
            None => PartyRef::Param(definition.identifying_param().ok_or_else(|| {
                SimulatorError::ObligationParty {
                    cell: cell.to_string(),
                    besluit: definition.name.clone(),
                    origin: origin.describe(),
                    role: SCHULDEISER.to_string(),
                    reason: format!(
                        "de verplichting laat hem weg, en zaakkenmerk '{}' wijst niet \
                         precies één parameter aan; declareer `{SCHULDEISER}: $parameter`",
                        definition.zaakkenmerk
                    ),
                }
            })?),
        };
        Ok((schuldenaar, schuldeiser))
    }

    /// De rechtsverhouding die deze verplichting op dit bedrag oplevert.
    ///
    /// Hier valt de beslissing over een **negatief** bedrag. Een negatieve
    /// betaling bestaat niet: wat een vaststelling lager dan het voorschot
    /// oplevert, is een verplichting de andere kant op (Awb 4:57), met de partij
    /// als schuldenaar en een positief bedrag. Dat is een andere rechtsverhouding
    /// en geen minteken, dus de wet moet hem declareren — zwijgt ze, dan valt het
    /// besluit hier om en wordt er niets vastgelegd.
    fn relation(
        &self,
        cell: &str,
        definition: &BesluitDefinition,
        declared: &DeclaredObligations,
        params: &BTreeMap<String, Value>,
        total: Decimal,
    ) -> Result<ObligationRelation> {
        let (schuldenaar, schuldeiser) = self.parties(cell, definition, &declared.origin)?;
        let schuldenaar = schuldenaar.resolve(SCHULDENAAR, cell, definition, declared, params)?;
        let schuldeiser = schuldeiser.resolve(SCHULDEISER, cell, definition, declared, params)?;
        // `validate` heeft de soort al aan [`BETALING`] gebonden; wat er anders
        // uit kan komen, komt uit de richting en niet uit de declaratie.
        let soort = ObligationKind::Betaling;
        if total >= Decimal::ZERO {
            return Ok(ObligationRelation {
                soort,
                schuldenaar,
                schuldeiser,
                total,
            });
        }
        match self.richting_bij_negatief {
            Some(RichtingBijNegatief::Omkeren) => Ok(ObligationRelation {
                soort: soort.reversed(),
                schuldenaar: schuldeiser,
                schuldeiser: schuldenaar,
                total: -total,
            }),
            None => Err(SimulatorError::NegativeObligationAmount {
                cell: cell.to_string(),
                besluit: definition.name.clone(),
                origin: declared.origin.describe(),
                output: self.bedrag.clone(),
                bedrag: total.to_string(),
            }),
        }
    }

    /// Controleer deze verplichting tegen het besluit dat haar artikel uitvoert.
    ///
    /// Alles wat zonder de wereld te beantwoorden valt staat hier: de soort moet
    /// bestaan, het bedrag moet een uitkomst zijn die het besluit vastlegt, een
    /// letterlijk ritme moet bestaan, en `vanaf` moet een datum kunnen
    /// opleveren. Wat hier *niet* kan — bestaat de instelling, is er een cel aan
    /// het bevoegd gezag gebonden — toetst de wereld.
    fn validate(
        &self,
        cell: &str,
        definition: &BesluitDefinition,
        origin: &ObligationOrigin,
        article_outputs: &BTreeSet<String>,
        stage_outputs: &BTreeSet<String>,
    ) -> Result<()> {
        let besluit = definition.name.as_str();
        let outputs = definition.recorded_outputs();
        let params = definition.params.as_slice();
        if self.soort != BETALING {
            return Err(SimulatorError::UnknownObligationKind {
                cell: cell.to_string(),
                besluit: besluit.to_string(),
                origin: origin.describe(),
                soort: self.soort.clone(),
                known: BETALING.to_string(),
            });
        }

        // De twee partijen: allebei te lezen, en een `$parameter` moet bij dit
        // besluit gedocumenteerd zijn. Hier en niet pas bij het besluit, want een
        // verplichting die naar een onbekende parameter wijst, legt iets op aan
        // niemand.
        let (schuldenaar, schuldeiser) = self.parties(cell, definition, origin)?;
        for (role, party) in [(SCHULDENAAR, schuldenaar), (SCHULDEISER, schuldeiser)] {
            if let PartyRef::Param(name) = party {
                if !documents(params, name) {
                    return Err(SimulatorError::ObligationParty {
                        cell: cell.to_string(),
                        besluit: besluit.to_string(),
                        origin: origin.describe(),
                        role: role.to_string(),
                        reason: format!(
                            "'${name}': dit besluit documenteert die parameter niet \
                             (wel: {})",
                            parameter_listing(params)
                        ),
                    });
                }
            }
        }

        // Een uitkomst van dít artikel, of een uitkomst die het besluit erbij
        // vastlegt. Het eerste is het gewone geval — het bedrag staat in het
        // artikel dat de verplichting oplegt — en het tweede laat een besluit een
        // bedrag aanwijzen dat een ander artikel van dezelfde regeling uitrekent.
        let amount_error = || SimulatorError::ObligationAmount {
            cell: cell.to_string(),
            besluit: besluit.to_string(),
            origin: origin.describe(),
            amount: self.bedrag.clone(),
            outputs: article_outputs
                .iter()
                .map(String::as_str)
                .chain(outputs.iter().copied())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", "),
        };
        let output = self.bedrag.strip_prefix('$').ok_or_else(amount_error)?;
        if !outputs.contains(output) && !article_outputs.contains(output) {
            return Err(amount_error());
        }

        // Een `$instelling` valt hier niet na te kijken; een letterlijk ritme
        // wel, en dan hoort een typfout hier te vallen en niet bij het besluit.
        if !self.ritme.starts_with('$') && Schedule::from_name(&self.ritme).is_none() {
            return Err(SimulatorError::UnknownSchedule {
                cell: cell.to_string(),
                besluit: besluit.to_string(),
                schedule: self.ritme.clone(),
                known: Schedule::listing(),
            });
        }

        let vervaldatum_error = |reason: String| SimulatorError::ObligationVervaldatum {
            cell: cell.to_string(),
            besluit: besluit.to_string(),
            origin: origin.describe(),
            reason,
        };
        let from_bekendmaking = self.vanaf.as_deref() == Some(VANAF_BEKENDMAKING);
        match (&self.vervaldatum, from_bekendmaking) {
            // De gebeurtenis en niet een datum: er valt geen sjabloon in te
            // vullen en geen dag uit te rekenen, want de dag van de bekendmaking
            // staat bij het besluit nog niet vast. Wélke uitkomst van de
            // bekendmaking haar levert, moet de verplichting dan zelf zeggen —
            // en die uitkomst moet er bij die stage ook uit kunnen komen.
            (None, true) => {
                return Err(vervaldatum_error(format!(
                    "`vanaf: {VANAF_BEKENDMAKING}` hoort te noemen welke uitkomst van de \
                     bekendmaking de vervaldag levert (`vervaldatum: <uitkomst>`); de stage kan \
                     leveren: {}",
                    listing(stage_outputs)
                )))
            }
            (Some(output), true) => {
                if !stage_outputs.contains(output) {
                    return Err(vervaldatum_error(format!(
                        "'{output}' is geen uitkomst die de bekendmaking van dit besluit oplevert \
                         (wel: {})",
                        listing(stage_outputs)
                    )));
                }
                return Ok(());
            }
            (Some(output), false) => {
                return Err(vervaldatum_error(format!(
                    "`vervaldatum: {output}` hoort bij `vanaf: {VANAF_BEKENDMAKING}`; zonder die \
                     gebeurtenis volgt de vervaldag uit `vanaf` en het ritme"
                )))
            }
            (None, false) => {}
        }
        let Some(from) = &self.vanaf else {
            return Ok(());
        };
        if !closing_braces_match(from) {
            return Err(SimulatorError::MalformedObligationDate {
                cell: cell.to_string(),
                besluit: besluit.to_string(),
                template: from.clone(),
                reason: "een accolade sluit niet".to_string(),
            });
        }

        let template = Template::parse(from);
        for part in &template.parts {
            if !documents(params, part.reference) {
                return Err(SimulatorError::UnknownReference {
                    cell: cell.to_string(),
                    subject: Subject::Besluit,
                    name: besluit.to_string(),
                    reference: part.reference.to_string(),
                });
            }
        }

        // Zonder verwijzingen staat de datum hier al vast, dus een sjabloon dat
        // nooit een datum kan opleveren hoort bij het optuigen te vallen en niet
        // pas bij het besluit dat erop rekent.
        if template.parts.is_empty() {
            parse_date(from).ok_or_else(|| SimulatorError::MalformedObligationDate {
                cell: cell.to_string(),
                besluit: besluit.to_string(),
                template: from.clone(),
                reason: "dat is geen datum (JJJJ-MM-DD)".to_string(),
            })?;
        }
        Ok(())
    }

    /// Het ritme van deze verplichting, met de instellingen van de wereld erbij.
    fn schedule(
        &self,
        cell: &str,
        besluit: &str,
        settings: &BTreeMap<String, Value>,
    ) -> Result<Schedule> {
        let name = match self.ritme.strip_prefix('$') {
            None => self.ritme.clone(),
            Some(setting) => settings
                .get(setting)
                .ok_or_else(|| SimulatorError::UnknownSetting {
                    cell: cell.to_string(),
                    besluit: besluit.to_string(),
                    setting: setting.to_string(),
                    known: settings
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                })?
                .to_string(),
        };

        Schedule::from_name(&name).ok_or_else(|| SimulatorError::UnknownSchedule {
            cell: cell.to_string(),
            besluit: besluit.to_string(),
            schedule: name,
            known: Schedule::listing(),
        })
    }

    /// De dag waarop de eerste termijn vervalt.
    ///
    /// Zonder `from` is dat het moment van het besluit. Met `from` is het wat
    /// het sjabloon oplevert, en dat mag vóór het besluit liggen: een besluit dat
    /// later genomen wordt dan het schema begint, is een te laat besluit en geen
    /// ongeldig besluit. De termijnen die daardoor al vervallen hadden moeten
    /// zijn, worden op de dag van het besluit ingehaald ([`inhalen`]) — niet hier,
    /// want de dag uit het schema hoort bij elke termijn in het gram te blijven
    /// staan. Wat `from` oplevert moet wél een datum zijn.
    ///
    /// Eén woord levert geen datum op maar een **gebeurtenis**:
    /// [`VANAF_BEKENDMAKING`]. Dan is er bij het besluit nog niets in te
    /// roosteren — zie [`ObligationStart::Bekendmaking`].
    fn start(
        &self,
        cell: &str,
        besluit: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<ObligationStart> {
        let Some(from) = &self.vanaf else {
            return Ok(ObligationStart::Op(op_moment));
        };
        if from == VANAF_BEKENDMAKING {
            return Ok(ObligationStart::Bekendmaking);
        }

        let filled = Template::parse(from).fill(params);
        let start = parse_date(&filled).ok_or_else(|| SimulatorError::MalformedObligationDate {
            cell: cell.to_string(),
            besluit: besluit.to_string(),
            template: from.clone(),
            reason: format!("ingevuld levert dat '{filled}' op, en dat is geen datum (JJJJ-MM-DD)"),
        })?;
        Ok(ObligationStart::Op(start))
    }

    /// Het bedrag waarover deze verplichting gaat, uit wat de uitvoering
    /// opleverde.
    fn total(
        &self,
        cell: &str,
        besluit: &str,
        outputs: &BTreeMap<String, Value>,
    ) -> Result<Decimal> {
        // `validate` heeft de naam al aan een uitkomst van dit besluit gebonden;
        // met of zonder `$` wijst ze dezelfde uitkomst aan.
        let output = self.bedrag.strip_prefix('$').unwrap_or(&self.bedrag);
        let found = outputs.get(output);
        found
            .and_then(Value::as_decimal)
            .ok_or_else(|| SimulatorError::ObligationAmountValue {
                cell: cell.to_string(),
                besluit: besluit.to_string(),
                output: output.to_string(),
                found: found.map_or_else(
                    || "niet uitgerekend door deze uitvoering".to_string(),
                    |value| format!("{} ({})", value, value.type_name()),
                ),
            })
    }
}

/// Haal een termijn in die vóór `inhaaldag` zou vervallen.
///
/// Een betaling kan er niet eerder zijn dan het besluit waaruit ze volgt, en
/// niet eerder dan de bekendmaking als de verplichting daarop wacht (Awb 4:86,
/// 4:87). Ligt de dag uit het schema daarvoor, dan vervalt de termijn op
/// `inhaaldag` en geeft deze functie de oorspronkelijke dag erbij terug; anders
/// blijft de dag staan en is dat tweede deel `None`. Volgorde, volgnummer en
/// bedrag gaan hier niet doorheen en blijven dus wat ze waren.
///
/// Zo valt een te laat besluit niet om, en legt de klok ook niets vast op een
/// moment dat al geweest is: de inhaalbetaling ligt op de dag dat ze kon.
fn inhalen(vervaldatum: NaiveDate, inhaaldag: NaiveDate) -> (NaiveDate, Option<NaiveDate>) {
    if vervaldatum < inhaaldag {
        (inhaaldag, Some(vervaldatum))
    } else {
        (vervaldatum, None)
    }
}

/// Verdeel een bedrag over een aantal termijnen, zonder een cent te verliezen.
///
/// Elke termijn krijgt hetzelfde bedrag in hele eenheden (de eenheid van de wet
/// — in dit corpus de eurocent), en wat niet deelbaar is gaat naar de laatste.
/// De som van de termijnen is dus exact het bedrag waarover besloten is; dat is
/// de eigenschap die telt, want "betaald tot nu toe" wordt eroverheen
/// gesommeerd en moet aan het eind uitkomen op wat toegekend is.
fn split(total: Decimal, terms: u32) -> Vec<Decimal> {
    if terms <= 1 {
        return vec![total];
    }
    let each = (total / Decimal::from(terms)).trunc();
    let mut amounts = vec![each; (terms - 1) as usize];
    amounts.push(total - each * Decimal::from(terms - 1));
    amounts
}

/// Een datum uit een sjabloon, of `None` als het er geen is.
fn parse_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

/// Eén verwijzing uit een zaakkenmerk-sjabloon, met de letterlijke tekst erachter.
struct TemplatePart<'a> {
    /// De parameternaam tussen de accolades.
    reference: &'a str,
    /// Wat er letterlijk achter deze verwijzing staat, tot de volgende
    /// verwijzing of tot het eind.
    literal: &'a str,
}

/// Een zaakkenmerk-sjabloon, uit elkaar gehaald.
///
/// Eén parser voor het optuigen én het invullen: zouden die uit elkaar lopen,
/// dan zou een sjabloon dat bij het optuigen goedgekeurd is bij het besluit
/// iets anders opleveren dan de toets veronderstelde.
pub(crate) struct Template<'a> {
    /// De letterlijke tekst vóór de eerste verwijzing.
    leading: &'a str,
    /// De verwijzingen, in volgorde.
    parts: Vec<TemplatePart<'a>>,
}

impl<'a> Template<'a> {
    /// Haal een sjabloon uit elkaar.
    ///
    /// Een accolade die niet sluit levert geen verwijzing op; dat geval wordt bij
    /// het optuigen apart geweigerd ([`closing_braces_match`]), zodat het hier
    /// niet stil als letterlijke tekst hoeft te eindigen.
    pub(crate) fn parse(template: &'a str) -> Self {
        let (leading, mut rest) = match template.split_once('{') {
            Some((leading, rest)) => (leading, rest),
            None => {
                return Self {
                    leading: template,
                    parts: Vec::new(),
                }
            }
        };

        let mut parts = Vec::new();
        while let Some((reference, after)) = rest.split_once('}') {
            // Geen volgende `{` betekent dat de rest letterlijke tekst is; `rest`
            // wordt dan leeg en de lus stopt vanzelf op de volgende ronde.
            let (literal, remainder) = after.split_once('{').unwrap_or((after, ""));
            parts.push(TemplatePart { reference, literal });
            rest = remainder;
        }
        Self { leading, parts }
    }

    /// Vul het sjabloon in met deze parameters.
    ///
    /// Eén invuller voor het zaakkenmerk én voor de startdatum van een
    /// verplichting: twee sjablonen met dezelfde vorm horen niet op twee manieren
    /// ingevuld te worden. Een verwijzing zonder waarde levert niets op — dat
    /// geval is bij het optuigen al geweigerd, hier zou het een lege plek zijn.
    pub(crate) fn fill(&self, params: &BTreeMap<String, Value>) -> String {
        let mut out = String::from(self.leading);
        for part in &self.parts {
            if let Some(value) = params.get(part.reference) {
                out.push_str(&value.to_string());
            }
            out.push_str(part.literal);
        }
        out
    }

    /// De parameters waarnaar dit sjabloon verwijst, in volgorde.
    pub(crate) fn references(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.parts.iter().map(|part| part.reference)
    }

    /// De letterlijke stukken die twee verwijzingen uit elkaar houden.
    ///
    /// De tekst vóór de eerste en die ná de laatste verwijzing tellen niet mee:
    /// die staan vast en kunnen geen twee invullingen laten samenvallen.
    fn separators(&self) -> impl Iterator<Item = &'a str> + '_ {
        let last = self.parts.len().saturating_sub(1);
        self.parts[..last].iter().map(|part| part.literal)
    }

    /// Weiger een parameterwaarde waarin een scheidingsteken van dit sjabloon
    /// voorkomt.
    ///
    /// Het zaakkenmerk is waaronder een zaak terug te vinden is, dus twee zaken
    /// mogen er nooit één worden. Bij `{jaar}/{bsn}` zou `jaar = "2024/9"` met
    /// `bsn = "99993653"` hetzelfde kenmerk geven als `jaar = "2024"` met
    /// `bsn = "999993653"`, en dan levert een reductie op dat kenmerk het besluit
    /// over iemand anders. Zolang geen waarde een scheidingsteken bevat, is de
    /// invulling omkeerbaar en kan dat niet gebeuren.
    ///
    /// Een sjabloon met één verwijzing heeft geen scheidingstekens en weigert dus
    /// niets: daar is elke waarde ondubbelzinnig.
    fn check_separators_absent(
        &self,
        cell: &str,
        besluit: &str,
        template: &str,
        params: &BTreeMap<String, Value>,
    ) -> Result<()> {
        let separators: Vec<&str> = self
            .separators()
            .filter(|separator| !separator.is_empty())
            .collect();
        if separators.is_empty() {
            return Ok(());
        }

        for part in &self.parts {
            let Some(value) = params.get(part.reference) else {
                continue;
            };
            let text = value.to_string();
            for separator in &separators {
                if text.contains(separator) {
                    return Err(SimulatorError::ZaakkenmerkSeparatorInValue {
                        cell: cell.to_string(),
                        besluit: besluit.to_string(),
                        template: template.to_string(),
                        parameter: part.reference.to_string(),
                        separator: (*separator).to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Sluit elke `{` in dit sjabloon weer?
///
/// Apart van [`Template::parse`], omdat de parser een niet-sluitende accolade
/// overslaat: die twee moeten het eens zijn over wat een verwijzing is, en de
/// weigering hoort bij het optuigen te vallen.
pub(crate) fn closing_braces_match(template: &str) -> bool {
    let mut rest = template;
    while let Some((_, after)) = rest.split_once('{') {
        let Some((_, remainder)) = after.split_once('}') else {
            return false;
        };
        rest = remainder;
    }
    true
}

/// De afwijzingsvoorwaarden die deze uitkomsten vervullen.
///
/// Leeg is het gewone geval: geen voorwaarde gedeclareerd, of geen ervan
/// vervuld. Een uitkomst die de uitvoering niet opleverde — of die geen ja-of-nee
/// bleek — vervult niets: bij het optuigen is vastgesteld dát de regeling haar
/// als ja-of-nee kent, dus wat hier binnenkomt is een uitvoering die haar niet
/// gaf, en een afwijzing op een ontbrekend feit is precies het gat waarmee een
/// besluit niet hoort te rekenen.
///
/// `article_of` zoekt het artikel dat een uitkomst voortbrengt op: de versie
/// waarin gezocht moet worden hangt aan het moment van het besluit, en dat weet
/// alleen de aanroeper.
pub(crate) fn afwijzingsgronden(
    conditions: &BTreeMap<String, bool>,
    outputs: &BTreeMap<String, Value>,
    article_of: impl Fn(&str) -> Option<String>,
) -> Vec<Afwijzingsgrond> {
    conditions
        .iter()
        .filter(|(output, expected)| {
            outputs.get(*output).and_then(Value::as_bool) == Some(**expected)
        })
        .map(|(output, expected)| Afwijzingsgrond {
            output: output.clone(),
            value: *expected,
            article: article_of(output),
        })
        .collect()
}

/// De veldnamen die de stroom met decretogrammen van deze cel gaat dragen.
///
/// Bekend vóór het eerste besluit, en dat moet ook: een lexostatus die over deze
/// stroom reduceert wordt bij het optuigen getoetst, en dan is de stroom nog
/// leeg. Zonder deze lijst zou elke reductie over een decretogram als typfout
/// geweigerd worden.
pub(crate) fn declared_fields(definitions: &[BesluitDefinition]) -> BTreeSet<String> {
    let mut fields: BTreeSet<String> = beschikkingen_fields()
        .into_iter()
        .map(str::to_string)
        .collect();
    for definition in definitions {
        fields.extend(
            definition
                .recorded_outputs()
                .into_iter()
                .map(str::to_string),
        );
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(yaml: &str) -> std::result::Result<BesluitInput, String> {
        serde_yaml_ng::from_str(yaml).map_err(|e| e.to_string())
    }

    #[test]
    fn een_input_uit_een_kroniek_wordt_gelezen() {
        let parsed = input("from_chronicle: inkomensleveringen\nfield: verzamelinkomen\n")
            .unwrap_or_else(|e| panic!("de kroniekvorm moet gelezen worden: {e}"));
        assert_eq!(
            parsed,
            BesluitInput::FromChronicle {
                chronicle: "inkomensleveringen".to_string(),
                field: "verzamelinkomen".to_string(),
            }
        );
    }

    #[test]
    fn een_input_uit_een_parameter_wordt_gelezen() {
        let parsed =
            input("param: bsn\n").unwrap_or_else(|e| panic!("de parametervorm moet lezen: {e}"));
        assert_eq!(
            parsed,
            BesluitInput::Param {
                param: "bsn".to_string()
            }
        );
    }

    #[test]
    fn een_input_met_twee_vormen_noemt_ze_beide() {
        let err = input("from_chronicle: relaties\nfield: x\nparam: bsn\n")
            .expect_err("twee vormen in één input hoort te falen");
        assert!(
            err.contains("relaties") && err.contains("bsn"),
            "de melding moet beide vormen noemen, kreeg: {err}"
        );
    }

    #[test]
    fn een_input_uit_een_kroniek_zonder_veld_wordt_geweigerd() {
        let err =
            input("from_chronicle: relaties\n").expect_err("zonder `field` hoort het te falen");
        assert!(
            err.contains("`field`"),
            "de melding moet `field` noemen, kreeg: {err}"
        );
    }

    #[test]
    fn een_input_zonder_vorm_noemt_de_vier_vormen() {
        let err = input("{}\n").expect_err("een input zonder vorm hoort te falen");
        assert!(
            err.contains("`from_chronicle`")
                && err.contains("`param`")
                && err.contains("`accept_from`")
                && err.contains("`from_decretogram`"),
            "de melding moet vertellen welke vormen er zijn, kreeg: {err}"
        );
    }

    #[test]
    fn een_input_uit_een_eerder_besluit_wordt_gelezen() {
        let parsed = input("from_decretogram: toekenning\nfield: hoogte_zorgtoeslag\n")
            .unwrap_or_else(|e| panic!("de teruglees-vorm moet gelezen worden: {e}"));
        assert_eq!(
            parsed,
            BesluitInput::FromDecretogram {
                besluit: "toekenning".to_string(),
                field: "hoogte_zorgtoeslag".to_string(),
            }
        );
        assert_eq!(
            parsed.accepts_from(),
            None,
            "teruglezen is geen vraag over een celgrens"
        );
    }

    #[test]
    fn een_input_uit_een_eerder_besluit_zonder_veld_wordt_geweigerd() {
        let err =
            input("from_decretogram: toekenning\n").expect_err("zonder `field` hoort het te falen");
        assert!(
            err.contains("`field`") && err.contains("toekenning"),
            "de melding moet zeggen wat er mist en waaruit, kreeg: {err}"
        );
    }

    /// Teruglezen gebeurt over de eigen kroniek en niet over een celgrens; een
    /// `lexostatus` ernaast leest als een vraag aan een ander en is er geen.
    #[test]
    fn een_teruglezing_met_een_veld_van_een_andere_vorm_wordt_geweigerd() {
        let err = input("from_decretogram: toekenning\nfield: bedrag\nlexostatus: beschikking\n")
            .expect_err("een veld van een andere vorm hoort te falen");
        assert!(
            err.contains("`lexostatus`") && err.contains("from_decretogram"),
            "de melding moet zeggen welk veld niet bij welke vorm hoort, kreeg: {err}"
        );
    }

    #[test]
    fn een_input_die_van_een_andere_cel_geaccepteerd_wordt_wordt_gelezen() {
        let parsed = input(
            "accept_from: belastingdienst\nlexostatus: toetsingsinkomen\n\
             field: toetsingsinkomen\nparams:\n  bsn: $bsn\n",
        )
        .unwrap_or_else(|e| panic!("de accepteervorm moet gelezen worden: {e}"));
        assert_eq!(
            parsed,
            BesluitInput::AcceptFrom {
                cell: "belastingdienst".to_string(),
                lexostatus: "toetsingsinkomen".to_string(),
                field: "toetsingsinkomen".to_string(),
                params: BTreeMap::from([("bsn".to_string(), "$bsn".to_string())]),
            }
        );
        assert_eq!(parsed.accepts_from(), Some("belastingdienst"));
    }

    #[test]
    fn accepteren_zonder_lexostatus_wordt_geweigerd() {
        // Een cel is alleen te bevragen langs een naam die ze publiceert; zonder
        // die naam is er geen vraag om te stellen.
        let err = input("accept_from: belastingdienst\nfield: toetsingsinkomen\n")
            .expect_err("zonder `lexostatus` hoort het te falen");
        assert!(
            err.contains("`lexostatus`") && err.contains("belastingdienst"),
            "de melding moet zeggen wat er mist en bij wie, kreeg: {err}"
        );
    }

    #[test]
    fn accepteren_zonder_veld_wordt_geweigerd() {
        let err = input("accept_from: belastingdienst\nlexostatus: toetsingsinkomen\n")
            .expect_err("zonder `field` hoort het te falen");
        assert!(
            err.contains("`field`"),
            "de melding moet `field` noemen, kreeg: {err}"
        );
    }

    /// Een `lexostatus` naast een `param` leest als een vraag over de celgrens en
    /// is er geen. Stil laten liggen zou een input opleveren die iets anders doet
    /// dan er staat.
    #[test]
    fn een_veld_van_een_andere_vorm_wordt_geweigerd() {
        let err = input("param: bsn\nlexostatus: toetsingsinkomen\n")
            .expect_err("een veld van een andere vorm hoort te falen");
        assert!(
            err.contains("`lexostatus`") && err.contains("param"),
            "de melding moet zeggen welk veld niet bij welke vorm hoort, kreeg: {err}"
        );
    }

    #[test]
    fn accepteren_van_de_eigen_cel_wordt_geweigerd() {
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.inputs.insert(
            "toetsingsinkomen".to_string(),
            BesluitInput::AcceptFrom {
                cell: "toeslagen".to_string(),
                lexostatus: "toetsingsinkomen".to_string(),
                field: "toetsingsinkomen".to_string(),
                params: BTreeMap::new(),
            },
        );

        let err = definition
            .validate_input(
                "toeslagen",
                &surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag"),
                "toetsingsinkomen",
                &definition.inputs["toetsingsinkomen"],
            )
            .expect_err("de eigen cel is geen peer");
        assert!(
            matches!(err, SimulatorError::AcceptFromSelf { .. }),
            "verwachtte AcceptFromSelf, kreeg {err}"
        );
    }

    #[test]
    fn een_verwijzing_in_de_parameters_van_een_accepteervraag_moet_gedocumenteerd_zijn() {
        let definition = definition("zorgtoeslag/{bsn}");
        let origin = BesluitInput::AcceptFrom {
            cell: "belastingdienst".to_string(),
            lexostatus: "toetsingsinkomen".to_string(),
            field: "toetsingsinkomen".to_string(),
            params: BTreeMap::from([("bsn".to_string(), "$burgerservicenummer".to_string())]),
        };

        let err = definition
            .validate_input(
                "toeslagen",
                &surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag"),
                "toetsingsinkomen",
                &origin,
            )
            .expect_err("een verwijzing zonder gedocumenteerde parameter hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownReference { .. }),
            "verwachtte UnknownReference, kreeg {err}"
        );
    }

    /// Teruglezen uit een naam die de cel niet als besluit kent, faalt bij het
    /// optuigen: anders zou elke zaak "geen eerder besluit" opleveren en is een
    /// typfout niet van een lege geschiedenis te onderscheiden.
    #[test]
    fn terug_lezen_uit_een_onbekend_besluit_wordt_geweigerd() {
        let definition = definition("zorgtoeslag/{bsn}");
        let origin = BesluitInput::FromDecretogram {
            besluit: "verlening".to_string(),
            field: "heeft_recht_op_zorgtoeslag".to_string(),
        };

        let err = definition
            .validate_input(
                "toeslagen",
                &surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag"),
                "is_verzekerde",
                &origin,
            )
            .expect_err("een besluit dat de cel niet kent hoort te falen");
        let SimulatorError::UnknownEarlierBesluit { known, .. } = &err else {
            panic!("verwachtte UnknownEarlierBesluit, kreeg {err}");
        };
        assert_eq!(known, "toekenning");
    }

    /// Welke velden een gram draagt staat vast zodra de definities er zijn: de
    /// uitkomsten, de vaste velden en de inputs waarop het besluit rekende.
    #[test]
    fn terug_lezen_kent_de_uitkomsten_en_de_vaste_velden_van_dat_gram() {
        let definition = definition("zorgtoeslag/{bsn}");
        let surface = surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag");
        let read = |field: &str| {
            definition.validate_input(
                "toeslagen",
                &surface,
                "is_verzekerde",
                &BesluitInput::FromDecretogram {
                    besluit: "toekenning".to_string(),
                    field: field.to_string(),
                },
            )
        };

        for field in ["heeft_recht_op_zorgtoeslag", ZAAKKENMERK, RECEIPT] {
            read(field).unwrap_or_else(|e| panic!("veld '{field}' hoort gelezen te mogen: {e}"));
        }

        let err = read("hoogte_zorgtoeslag")
            .expect_err("een veld dat zo'n gram niet draagt hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownEarlierBesluitField { .. }),
            "verwachtte UnknownEarlierBesluitField, kreeg {err}"
        );
    }

    /// De inputs waarop dat besluit rekende horen er ook bij: die liggen in het
    /// gram een laag dieper, maar `from_decretogram` kan ze lezen.
    #[test]
    fn terug_lezen_kent_ook_de_inputs_waarop_dat_gram_rekende() {
        let mut earlier = definition("zorgtoeslag/{bsn}");
        earlier.inputs.insert(
            "is_verzekerde".to_string(),
            BesluitInput::FromChronicle {
                chronicle: "inkomensleveringen".to_string(),
                field: "is_verzekerde".to_string(),
            },
        );
        let mut surface = surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag");
        surface
            .besluit_fields
            .insert("toekenning".to_string(), earlier.gram_fields());

        definition("zorgtoeslag/{bsn}")
            .validate_input(
                "toeslagen",
                &surface,
                "is_verzekerde",
                &BesluitInput::FromDecretogram {
                    besluit: "toekenning".to_string(),
                    field: "is_verzekerde".to_string(),
                },
            )
            .unwrap_or_else(|e| panic!("een input van dat gram hoort gelezen te mogen: {e}"));
    }

    /// Draagt zo'n gram één naam in beide lagen — als vast veld of uitkomst én
    /// als input — dan wijst `field` twee waarden aan. Welke van de twee gepakt
    /// wordt, is niets om te raden: bij het optuigen geweigerd, net als een
    /// uitkomst die een vast veld zou overschrijven.
    #[test]
    fn terug_lezen_van_een_naam_die_het_gram_twee_keer_draagt_wordt_geweigerd() {
        let mut earlier = definition("zorgtoeslag/{bsn}");
        earlier.inputs.insert(
            ZAAKKENMERK.to_string(),
            BesluitInput::Param {
                param: "bsn".to_string(),
            },
        );
        let mut surface = surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag");
        surface
            .besluit_fields
            .insert("toekenning".to_string(), earlier.gram_fields());

        let err = definition("zorgtoeslag/{bsn}")
            .validate_input(
                "toeslagen",
                &surface,
                "is_verzekerde",
                &BesluitInput::FromDecretogram {
                    besluit: "toekenning".to_string(),
                    field: ZAAKKENMERK.to_string(),
                },
            )
            .expect_err("een naam die twee waarden aanwijst hoort te falen");
        assert!(
            matches!(err, SimulatorError::AmbiguousEarlierBesluitField { .. }),
            "verwachtte AmbiguousEarlierBesluitField, kreeg {err}"
        );
    }

    /// `$zaakkenmerk` is geen parameter en hoeft dus niet gedocumenteerd te zijn:
    /// het komt uit het eigen sjabloon van de definitie.
    #[test]
    fn het_zaakkenmerk_mag_in_de_vraag_over_de_celgrens() {
        let definition = definition("zorgtoeslag/{bsn}");
        let origin = BesluitInput::AcceptFrom {
            cell: "belastingdienst".to_string(),
            lexostatus: "betaald_tot_nu_toe".to_string(),
            field: "bedrag".to_string(),
            params: BTreeMap::from([(
                "zaakkenmerk".to_string(),
                ZAAKKENMERK_REFERENCE.to_string(),
            )]),
        };

        definition
            .validate_input(
                "toeslagen",
                &surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag"),
                "toetsingsinkomen",
                &origin,
            )
            .unwrap_or_else(|e| {
                panic!("een verwijzing naar het eigen kenmerk hoort te mogen: {e}")
            });
    }

    /// Zonder sjabloon valt er niets in te vullen, en zou de bevraagde cel een
    /// lege tekst als zaak krijgen.
    #[test]
    fn het_zaakkenmerk_in_de_vraag_zonder_sjabloon_wordt_geweigerd() {
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.zaakkenmerk = String::new();
        let origin = BesluitInput::AcceptFrom {
            cell: "belastingdienst".to_string(),
            lexostatus: "betaald_tot_nu_toe".to_string(),
            field: "bedrag".to_string(),
            params: BTreeMap::from([(
                "zaakkenmerk".to_string(),
                ZAAKKENMERK_REFERENCE.to_string(),
            )]),
        };

        let err = definition
            .validate_input(
                "toeslagen",
                &surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag"),
                "toetsingsinkomen",
                &origin,
            )
            .expect_err("zonder sjabloon is er geen kenmerk om mee te sturen");
        assert!(
            matches!(
                err,
                SimulatorError::ZaakkenmerkReferenceWithoutTemplate { .. }
            ),
            "verwachtte ZaakkenmerkReferenceWithoutTemplate, kreeg {err}"
        );
    }

    /// Eén naam kan niet twee dingen betekenen: het ingevulde kenmerk van dit
    /// besluit, én wat de aanroeper meegaf.
    #[test]
    fn het_zaakkenmerk_naast_een_parameter_met_die_naam_wordt_geweigerd() {
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.params.push(DocumentedParameter {
            name: ZAAKKENMERK.to_string(),
            value_type: crate::cell::ParameterType::String,
            prefill: None,
        });
        let origin = BesluitInput::AcceptFrom {
            cell: "belastingdienst".to_string(),
            lexostatus: "betaald_tot_nu_toe".to_string(),
            field: "bedrag".to_string(),
            params: BTreeMap::from([(
                "zaakkenmerk".to_string(),
                ZAAKKENMERK_REFERENCE.to_string(),
            )]),
        };

        let err = definition
            .validate_input(
                "toeslagen",
                &surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag"),
                "toetsingsinkomen",
                &origin,
            )
            .expect_err("dezelfde naam voor twee dingen hoort te falen");
        assert!(
            matches!(err, SimulatorError::AmbiguousZaakkenmerkReference { .. }),
            "verwachtte AmbiguousZaakkenmerkReference, kreeg {err}"
        );
    }

    #[test]
    fn de_verzoeken_van_een_besluit_vullen_hun_verwijzingen_in() {
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.inputs.insert(
            "toetsingsinkomen".to_string(),
            BesluitInput::AcceptFrom {
                cell: "belastingdienst".to_string(),
                lexostatus: "toetsingsinkomen".to_string(),
                field: "toetsingsinkomen".to_string(),
                params: BTreeMap::from([("bsn".to_string(), "$bsn".to_string())]),
            },
        );
        definition.inputs.insert(
            "is_verzekerde".to_string(),
            BesluitInput::Param {
                param: "bsn".to_string(),
            },
        );

        let params = BTreeMap::from([
            ("bsn".to_string(), Value::String("999993653".to_string())),
            ("jaar".to_string(), Value::String("2024".to_string())),
        ]);
        let requests = definition.acceptance_requests(&params, "zorgtoeslag/999993653");

        assert_eq!(
            requests,
            vec![AcceptanceRequest {
                input: "toetsingsinkomen".to_string(),
                cell: "belastingdienst".to_string(),
                lexostatus: "toetsingsinkomen".to_string(),
                field: "toetsingsinkomen".to_string(),
                params: BTreeMap::from([(
                    "bsn".to_string(),
                    Value::String("999993653".to_string())
                )]),
            }],
            "alleen de accepteervorm levert een verzoek, met de waarde erin"
        );
    }

    /// `$zaakkenmerk` levert het ingevulde kenmerk van dít besluit, en niet de
    /// tekst `$zaakkenmerk` of een lege plek.
    #[test]
    fn een_verzoek_kan_naar_het_eigen_zaakkenmerk_verwijzen() {
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.inputs.insert(
            "betaald".to_string(),
            BesluitInput::AcceptFrom {
                cell: "belastingdienst".to_string(),
                lexostatus: "betaald_tot_nu_toe".to_string(),
                field: "bedrag".to_string(),
                params: BTreeMap::from([
                    ("zaakkenmerk".to_string(), ZAAKKENMERK_REFERENCE.to_string()),
                    ("jaar".to_string(), "$jaar".to_string()),
                ]),
            },
        );

        let params = BTreeMap::from([
            ("bsn".to_string(), Value::String("999993653".to_string())),
            ("jaar".to_string(), Value::String("2024".to_string())),
        ]);
        let requests = definition.acceptance_requests(&params, "zorgtoeslag/999993653");

        assert_eq!(
            requests
                .first()
                .map(|request| request.params.clone())
                .unwrap_or_default(),
            BTreeMap::from([
                (
                    "zaakkenmerk".to_string(),
                    Value::String("zorgtoeslag/999993653".to_string())
                ),
                ("jaar".to_string(), Value::String("2024".to_string())),
            ]),
            "de ingebouwde verwijzing en een gewone parameter horen naast elkaar te werken"
        );
    }

    fn definition(zaakkenmerk: &str) -> BesluitDefinition {
        serde_yaml_ng::from_str(&format!(
            r"
name: toekenning
regulation: wet_op_de_zorgtoeslag
output: heeft_recht_op_zorgtoeslag
zaakkenmerk: '{zaakkenmerk}'
params:
  - name: bsn
    type: string
  - name: jaar
    type: string
"
        ))
        .unwrap_or_else(|e| panic!("testdefinitie moet parsen: {e}"))
    }

    /// Vul een sjabloon in, of leg luid uit waarom dat niet mocht.
    fn zaakkenmerk(template: &str, params: &[(&str, &str)]) -> Result<String> {
        let params: BTreeMap<String, Value> = params
            .iter()
            .map(|(name, value)| ((*name).to_string(), Value::String((*value).to_string())))
            .collect();
        definition(template).zaakkenmerk("toeslagen", &params)
    }

    /// Wat een zaakkenmerk nog mist, is te vragen zonder het in te vullen.
    ///
    /// De droogloop van de beschikbaarheid leunt hierop: `fill` laat een
    /// verwijzing zonder waarde stil weg, dus zonder deze vraag zou een melding
    /// over een eerder besluit naar zaak `'/'` verwijzen — een kenmerk dat een
    /// lezer nergens terugvindt, in plaats van "de vraag noemt nog geen bsn".
    #[test]
    fn een_zaakkenmerk_noemt_de_verwijzingen_die_de_vraag_niet_invult() {
        let params: BTreeMap<String, Value> =
            BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))]);
        assert_eq!(
            definition("{jaar}/{bsn}").zaakkenmerk_gaps(&params),
            vec!["jaar".to_string()],
            "alleen wat er niet in staat, en in de volgorde van het sjabloon"
        );
        assert!(
            definition("zorgtoeslag/{bsn}")
                .zaakkenmerk_gaps(&params)
                .is_empty(),
            "een sjabloon waarvan elke verwijzing gevuld is, mist niets"
        );
    }

    #[test]
    fn het_zaakkenmerk_vult_de_parameters_in() {
        assert_eq!(
            zaakkenmerk("zorgtoeslag/{bsn}", &[("bsn", "999993653")])
                .unwrap_or_else(|e| panic!("het sjabloon moet in te vullen zijn: {e}")),
            "zorgtoeslag/999993653"
        );
    }

    /// Eén verwijzing kent geen scheidingsteken, dus elke waarde is eenduidig.
    ///
    /// Wat er vóór en achter staat ligt vast, dus `zorgtoeslag/` gevolgd door een
    /// waarde met een `/` erin blijft terug te lezen. Weigeren zou hier een regel
    /// opleggen die niets beschermt.
    #[test]
    fn een_enkele_verwijzing_neemt_elke_waarde_zoals_ze_is() {
        assert_eq!(
            zaakkenmerk("zorgtoeslag/{bsn}", &[("bsn", "9999/93653")])
                .unwrap_or_else(|e| panic!("één verwijzing hoort niets te weigeren: {e}")),
            "zorgtoeslag/9999/93653"
        );
    }

    /// Twee zaken mogen nooit één kenmerk krijgen.
    ///
    /// `{jaar}/{bsn}` met `2024/9` + `99993653` levert letterlijk hetzelfde
    /// kenmerk als `2024` + `999993653`. Dan wijst het kenmerk naar twee zaken en
    /// levert een reductie erop het besluit over iemand anders.
    #[test]
    fn een_waarde_met_het_scheidingsteken_erin_wordt_geweigerd() {
        let eerlijk = zaakkenmerk("{jaar}/{bsn}", &[("jaar", "2024"), ("bsn", "999993653")])
            .unwrap_or_else(|e| panic!("een gewone invulling moet slagen: {e}"));

        let err = zaakkenmerk("{jaar}/{bsn}", &[("jaar", "2024/9"), ("bsn", "99993653")])
            .expect_err("een waarde die het scheidingsteken bevat hoort te falen");
        assert!(
            matches!(err, SimulatorError::ZaakkenmerkSeparatorInValue { .. }),
            "verwachtte ZaakkenmerkSeparatorInValue, kreeg {err}"
        );
        assert_eq!(
            eerlijk, "2024/999993653",
            "de botsing die geweigerd wordt, is precies dit kenmerk"
        );
    }

    #[test]
    fn twee_verwijzingen_zonder_scheiding_worden_bij_het_optuigen_geweigerd() {
        let err = definition("{jaar}{bsn}")
            .validate_zaakkenmerk("toeslagen")
            .expect_err("twee verwijzingen tegen elkaar aan hoort te falen");
        assert!(
            matches!(err, SimulatorError::AdjacentZaakkenmerkReferences { .. }),
            "verwachtte AdjacentZaakkenmerkReferences, kreeg {err}"
        );
    }

    #[test]
    fn twee_verwijzingen_met_scheiding_mogen_wel() {
        definition("{jaar}/{bsn}")
            .validate_zaakkenmerk("toeslagen")
            .unwrap_or_else(|e| panic!("een gescheiden sjabloon hoort te mogen: {e}"));
    }

    #[test]
    fn een_zaakkenmerk_zonder_verwijzing_wordt_geweigerd() {
        let err = definition("zorgtoeslag")
            .validate_zaakkenmerk("toeslagen")
            .expect_err("een zaakkenmerk dat elke zaak gelijk maakt hoort te falen");
        assert!(
            matches!(err, SimulatorError::ZaakkenmerkWithoutReference { .. }),
            "verwachtte ZaakkenmerkWithoutReference, kreeg {err}"
        );
    }

    #[test]
    fn een_zaakkenmerk_met_een_open_accolade_wordt_geweigerd() {
        let err = definition("zorgtoeslag/{bsn")
            .validate_zaakkenmerk("toeslagen")
            .expect_err("een accolade die niet sluit hoort te falen");
        assert!(
            matches!(err, SimulatorError::MalformedZaakkenmerk { .. }),
            "verwachtte MalformedZaakkenmerk, kreeg {err}"
        );
    }

    /// Een oppervlak dat alles kent wat de definitie hieronder noemt.
    ///
    /// Rechtstreeks in elkaar gezet en niet uit een corpus gelezen: de botsing
    /// die deze test afdekt vraagt een regeling met een uitkomst die `receipt`
    /// heet, en die bestaat in het corpus niet. Dat ze er niet is, is geen reden
    /// om de weigering ongetest te laten — ze is er morgen misschien wel.
    fn surface_met_uitkomst<'a>(laws: &'a [String], output: &str) -> CellSurface<'a> {
        surface_met_type(laws, output, "boolean")
    }

    /// Hetzelfde oppervlak, met het type dat de regeling aan die uitkomst geeft.
    ///
    /// Apart, want `afwijzing_wanneer` toetst juist op dat type: een oppervlak
    /// dat alles als ja-of-nee doorgeeft, zou de weigering hieronder nooit
    /// kunnen laten vallen.
    fn surface_met_type<'a>(laws: &'a [String], output: &str, value_type: &str) -> CellSurface<'a> {
        CellSurface {
            laws,
            outputs: BTreeMap::from([(
                "wet_op_de_zorgtoeslag".to_string(),
                BTreeSet::from([output.to_string()]),
            )]),
            legal_characters: BTreeMap::from([(
                "wet_op_de_zorgtoeslag".to_string(),
                BTreeMap::from([(
                    output.to_string(),
                    BTreeSet::from([Some(BESCHIKKING.to_string())]),
                )]),
            )]),
            output_types: BTreeMap::from([(
                "wet_op_de_zorgtoeslag".to_string(),
                BTreeMap::from([(output.to_string(), BTreeSet::from([value_type.to_string()]))]),
            )]),
            afwijzing_blocks: BTreeMap::new(),
            regulation_inputs: BTreeMap::new(),
            streams: BTreeMap::new(),
            stream_keys: BTreeMap::new(),
            besluit_fields: BTreeMap::from([(
                "toekenning".to_string(),
                definition("zorgtoeslag/{bsn}").gram_fields(),
            )]),
            obligations: BTreeMap::new(),
        }
    }

    #[test]
    fn een_uitkomst_die_een_vast_veld_zou_overschrijven_wordt_geweigerd() {
        let laws = vec!["wet_op_de_zorgtoeslag".to_string()];
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.output = RECEIPT.to_string();

        let err = definition
            .validate("toeslagen", &surface_met_uitkomst(&laws, RECEIPT))
            .expect_err("een uitkomst die een vast veld overschrijft hoort te falen");
        assert!(
            matches!(err, SimulatorError::ReservedDecretogramField { .. }),
            "verwachtte ReservedDecretogramField, kreeg {err}"
        );
    }

    /// Een `afwijzing_wanneer`-blok zoals een artikel het declareert.
    fn blok(yaml: &str) -> Value {
        serde_yaml_ng::from_str(yaml).unwrap_or_else(|e| panic!("testblok moet parsen: {e}"))
    }

    /// Een oppervlak dat de regeling met dít blok op het artikel kent.
    fn surface_met_blok<'a>(
        laws: &'a [String],
        output: &str,
        value_type: &str,
        block: &str,
    ) -> CellSurface<'a> {
        let mut surface = surface_met_type(laws, output, value_type);
        surface.afwijzing_blocks = BTreeMap::from([(
            "wet_op_de_zorgtoeslag".to_string(),
            BTreeMap::from([(output.to_string(), vec![blok(block)])]),
        )]);
        surface
    }

    #[test]
    fn een_afwijzingsvoorwaarde_op_een_bekende_boolean_mag() {
        let laws = vec!["wet_op_de_zorgtoeslag".to_string()];
        definition("zorgtoeslag/{bsn}")
            .validate(
                "toeslagen",
                &surface_met_blok(
                    &laws,
                    "heeft_recht_op_zorgtoeslag",
                    "boolean",
                    "heeft_recht_op_zorgtoeslag: false",
                ),
            )
            .unwrap_or_else(|e| panic!("een voorwaarde op een bekende ja-of-nee moet mogen: {e}"));
    }

    #[test]
    fn een_afwijzingsvoorwaarde_op_een_onbekende_uitkomst_wordt_geweigerd() {
        let laws = vec!["wet_op_de_zorgtoeslag".to_string()];
        let err = definition("zorgtoeslag/{bsn}")
            .validate(
                "toeslagen",
                &surface_met_blok(
                    &laws,
                    "heeft_recht_op_zorgtoeslag",
                    "boolean",
                    "is_landelijke_partij: false",
                ),
            )
            .expect_err("een voorwaarde op een uitkomst die de regeling niet kent hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownOutput { .. }),
            "verwachtte UnknownOutput, kreeg {err}"
        );
    }

    #[test]
    fn een_afwijzingsvoorwaarde_op_een_niet_boolean_uitkomst_wordt_geweigerd() {
        let laws = vec!["wet_op_de_zorgtoeslag".to_string()];
        let err = definition("zorgtoeslag/{bsn}")
            .validate(
                "toeslagen",
                &surface_met_blok(
                    &laws,
                    "heeft_recht_op_zorgtoeslag",
                    "amount",
                    "heeft_recht_op_zorgtoeslag: false",
                ),
            )
            .expect_err("een voorwaarde op een bedrag raakt nooit vervuld en hoort te falen");
        assert!(
            matches!(err, SimulatorError::AfwijzingsvoorwaardeNotBoolean { .. }),
            "verwachtte AfwijzingsvoorwaardeNotBoolean, kreeg {err}"
        );
    }

    /// Een blok dat niet te lezen is, hoort bij het optuigen te vallen.
    ///
    /// Het staat in een **wet**, dus wie het schrijft is niet dezelfde als wie
    /// het leest; stil als "geen voorwaarde" eindigen zou de weigering uitzetten
    /// zonder dat er iets te zien is.
    #[test]
    fn een_onleesbaar_blok_wordt_geweigerd() {
        let laws = vec!["wet_op_de_zorgtoeslag".to_string()];
        let err = definition("zorgtoeslag/{bsn}")
            .validate(
                "toeslagen",
                &surface_met_blok(
                    &laws,
                    "heeft_recht_op_zorgtoeslag",
                    "boolean",
                    "- heeft_recht_op_zorgtoeslag",
                ),
            )
            .expect_err("een blok dat geen voorwaarde is hoort te falen");
        assert!(
            matches!(err, SimulatorError::MalformedAfwijzingWanneer { .. }),
            "verwachtte MalformedAfwijzingWanneer, kreeg {err}"
        );
    }

    /// Wanneer een besluit afwijst, hoort in de wet en niet in het wereldbestand.
    #[test]
    fn een_besluit_definitie_mag_niet_zelf_afwijzen() {
        let laws = vec!["wet_op_de_zorgtoeslag".to_string()];
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.afwijzing_wanneer = Some(blok("heeft_recht_op_zorgtoeslag: false"));

        let err = definition
            .validate(
                "toeslagen",
                &surface_met_uitkomst(&laws, "heeft_recht_op_zorgtoeslag"),
            )
            .expect_err("een wereldbestand dat zelf afwijst hoort te falen");
        assert!(
            matches!(err, SimulatorError::AfwijzingWanneerInWereldbestand { .. }),
            "verwachtte AfwijzingWanneerInWereldbestand, kreeg {err}"
        );
        assert!(
            err.to_string().contains("produces.extensions.chronolex"),
            "de melding hoort de weg te wijzen naar het blok in de regeling: {err}"
        );
    }

    #[test]
    fn een_vervulde_voorwaarde_levert_een_grond_met_haar_artikel() {
        let outputs =
            BTreeMap::from([("heeft_recht_op_zorgtoeslag".to_string(), Value::Bool(false))]);
        let conditions = BTreeMap::from([("heeft_recht_op_zorgtoeslag".to_string(), false)]);
        assert_eq!(
            afwijzingsgronden(&conditions, &outputs, |_| Some("2".to_string())),
            vec![Afwijzingsgrond {
                output: "heeft_recht_op_zorgtoeslag".to_string(),
                value: false,
                article: Some("2".to_string()),
            }],
            "de grond noemt de uitkomst, de waarde die afwijst en het artikel erachter"
        );
    }

    #[test]
    fn een_onvervulde_voorwaarde_levert_geen_grond() {
        let outputs =
            BTreeMap::from([("heeft_recht_op_zorgtoeslag".to_string(), Value::Bool(true))]);
        let conditions = BTreeMap::from([("heeft_recht_op_zorgtoeslag".to_string(), false)]);
        assert!(
            afwijzingsgronden(&conditions, &outputs, |_| None).is_empty(),
            "wie recht heeft, wordt niet afgewezen"
        );
    }

    /// Een uitkomst die de uitvoering niet gaf, wijst niets af.
    ///
    /// Anders zou een besluit op een gat rekenen: "de uitkomst ontbreekt" is
    /// iets anders dan "de uitkomst is onwaar", en van die twee mag alleen de
    /// tweede tot een weigering leiden.
    #[test]
    fn een_ontbrekende_uitkomst_wijst_niets_af() {
        let conditions = BTreeMap::from([("heeft_recht_op_zorgtoeslag".to_string(), false)]);
        assert!(
            afwijzingsgronden(&conditions, &BTreeMap::new(), |_| None).is_empty(),
            "een ontbrekende uitkomst is geen vervulde voorwaarde"
        );
    }

    /// Een verplichting zoals ze in een lexogram staat.
    fn obligation(yaml: &str) -> ObligationDefinition {
        serde_yaml_ng::from_str(yaml)
            .unwrap_or_else(|e| panic!("testverplichting moet parsen: {e}"))
    }

    /// Een verplichting met het gewone bedrag en ritme, plus wat de test wil.
    fn betaling(extra: &str) -> ObligationDefinition {
        obligation(&format!(
            "soort: betaling\nbedrag: $hoogte_zorgtoeslag\nritme: ineens\ngrondslag: art. 1\n{extra}"
        ))
    }

    /// Het lexogram waaruit de verplichtingen hieronder komen.
    fn origin() -> ObligationOrigin {
        ObligationOrigin {
            regulation: "wet_op_de_zorgtoeslag".to_string(),
            valid_from: Some("2024-01-01".to_string()),
            article: "2".to_string(),
        }
    }

    /// Het besluit dat het artikel uitvoert: één parameter, één uitkomst.
    fn uitvoerend_besluit() -> BesluitDefinition {
        serde_yaml_ng::from_str(
            r"
name: toekenning
regulation: wet_op_de_zorgtoeslag
output: hoogte_zorgtoeslag
zaakkenmerk: 'zorgtoeslag/{jaar}'
params:
  - name: jaar
    type: string
",
        )
        .unwrap_or_else(|e| panic!("testdefinitie moet parsen: {e}"))
    }

    /// De uitkomsten en parameters van het besluit dat het artikel uitvoert.
    fn valideer(obligation: &ObligationDefinition) -> Result<()> {
        obligation.validate(
            "toeslagen",
            &uitvoerend_besluit(),
            &origin(),
            &BTreeSet::from(["verzamelinkomen".to_string()]),
            &BTreeSet::from(["uiterste_betaaldatum_4_87".to_string()]),
        )
    }

    /// De rechtsverhouding die deze verplichting op dit bedrag oplevert.
    ///
    /// Recht op de naad waar de richting valt, en met opzet zonder wereld: wat
    /// het teken van het bedrag betekent, hangt niet af van de vorm van de input
    /// waaruit de uitkomst kwam. Een `param`, een `from_chronicle`, een
    /// `from_decretogram` en een `accept_from` komen alle vier als uitkomst uit
    /// dezelfde uitvoering, en dit is het enige punt waar er daarna nog naar het
    /// teken gekeken wordt.
    fn verhouding(obligation: &ObligationDefinition, bedrag: &str) -> Result<ObligationRelation> {
        let definition = uitvoerend_besluit();
        let declared = DeclaredObligations {
            origin: origin(),
            authority: Some("Dienst Toeslagen".to_string()),
            article_outputs: BTreeSet::from(["hoogte_zorgtoeslag".to_string()]),
            stage_outputs: BTreeSet::new(),
            items: vec![obligation.clone()],
            vervanging: None,
        };
        let params = BTreeMap::from([("jaar".to_string(), Value::String("999993653".to_string()))]);
        let total = Decimal::from_str_exact(bedrag)
            .unwrap_or_else(|e| panic!("testbedrag '{bedrag}' moet leesbaar zijn: {e}"));
        obligation.relation("toeslagen", &definition, &declared, &params, total)
    }

    /// **Een negatief bedrag zonder declaratie komt hier niet voorbij.**
    ///
    /// De weigering zit op de naad tussen het uitgerekende bedrag en de termijn,
    /// en niet bij een van de inputvormen: wat de uitvoering oplevert is één
    /// uitkomstenkaart, en of die uit een parameter, een eigen kroniek, een
    /// eerder gram of een andere cel gevoed werd, is hier niet meer te zien. Er
    /// is dan ook maar één plek waar een [`ObligationDue`] ontstaat, en die komt
    /// hier langs.
    #[test]
    fn een_negatief_bedrag_zonder_declaratie_levert_geen_verhouding() {
        let err = verhouding(&betaling(""), "-48602")
            .expect_err("een negatief bedrag zonder declaratie hoort te falen");
        assert!(
            matches!(err, SimulatorError::NegativeObligationAmount { .. }),
            "verwachtte NegativeObligationAmount, kreeg {err}"
        );
    }

    /// Met de declaratie wisselen de partijen en wordt het bedrag positief.
    #[test]
    fn een_negatief_bedrag_met_omkeren_draait_de_verhouding_om() {
        let relation = verhouding(&betaling("richting_bij_negatief: omkeren\n"), "-48602")
            .unwrap_or_else(|e| panic!("met de declaratie hoort dit te mogen: {e}"));
        assert_eq!(relation.soort, ObligationKind::Terugvordering);
        assert_eq!(relation.schuldenaar, "999993653");
        assert_eq!(relation.schuldeiser, "Dienst Toeslagen");
        assert_eq!(relation.total, Decimal::from(48602));
    }

    /// En nul is geen omkering: er valt niets terug te vorderen, dus het blijft
    /// een betaling met de partijen zoals de wet ze aanwees.
    #[test]
    fn een_bedrag_van_nul_blijft_een_betaling() {
        let relation = verhouding(&betaling("richting_bij_negatief: omkeren\n"), "0")
            .unwrap_or_else(|e| panic!("nul hoort te mogen: {e}"));
        assert_eq!(relation.soort, ObligationKind::Betaling);
        assert_eq!(relation.schuldenaar, "Dienst Toeslagen");
        assert_eq!(relation.schuldeiser, "999993653");
    }

    /// Een artikel zoals het in een regeling staat.
    fn artikel(block: &str) -> Article {
        serde_yaml_ng::from_str(&format!(
            "number: '2'\nurl: https://example.com/wet#Artikel2\ntext: tekst\n\
             machine_readable:\n  execution:\n    produces:\n      \
             legal_character: BESCHIKKING\n{block}"
        ))
        .unwrap_or_else(|e| panic!("testartikel moet parsen: {e}"))
    }

    /// Een artikel zonder blok legt niets op, en dat is geen fout: niet elke
    /// beschikking kent een bedrag toe.
    #[test]
    fn een_artikel_zonder_blok_legt_niets_op() {
        let declared = DeclaredObligations::from_article(origin(), None, &artikel(""))
            .unwrap_or_else(|e| panic!("een artikel zonder blok hoort te mogen: {e}"));
        assert!(declared.is_empty(), "er valt niets na te komen");
    }

    /// Een blok dat er wél staat maar niet klopt, is een fout in de **wet**. Stil
    /// overslaan zou een regeling laten zwijgen waar ze spreekt, en dan zou een
    /// besluit zonder verplichting niets bijzonders lijken.
    #[test]
    fn een_chronolex_blok_dat_niet_klopt_wordt_geweigerd() {
        for block in [
            // Een typfout in de sleutel: `deny_unknown_fields` op het blok.
            "      extensions:\n        chronolex:\n          verplichtignen: []\n",
            // Een verplichting zonder grondslag: die is verplicht.
            "      extensions:\n        chronolex:\n          verplichtingen:\n            - soort: betaling\n              bedrag: $bedrag\n              ritme: ineens\n",
            // Het blok is geen mapping.
            "      extensions:\n        chronolex: []\n",
        ] {
            let err = DeclaredObligations::from_article(origin(), None, &artikel(block))
                .expect_err("een blok dat niet klopt hoort te falen");
            assert!(
                matches!(err, SimulatorError::MalformedChronolexBlock { .. }),
                "verwachtte MalformedChronolexBlock, kreeg {err}"
            );
        }
    }

    /// De som van de termijnen is exact het bedrag waarover besloten is.
    ///
    /// Dat is de eigenschap waarop "betaald tot nu toe" rust: wie de rest laat
    /// vallen, betaalt aan het eind minder uit dan de beschikking toekende, en
    /// dat is dan nergens aan te zien behalve aan het totaal.
    #[test]
    fn de_termijnen_tellen_op_tot_het_hele_bedrag() {
        for bedrag in ["100000", "197205.31187", "1", "-4000"] {
            let total: Decimal = bedrag
                .parse()
                .unwrap_or_else(|e| panic!("testbedrag '{bedrag}' moet leesbaar zijn: {e}"));
            for terms in [1_u32, 4, 12] {
                let amounts = split(total, terms);
                assert_eq!(amounts.len(), terms as usize);
                assert_eq!(
                    amounts.iter().sum::<Decimal>(),
                    total,
                    "{bedrag} over {terms} termijnen hoort exact op te tellen"
                );
            }
        }
    }

    /// Gelijke termijnen, en het restant op de laatste. Zonder deze assertie zou
    /// een verdeling die alles op de eerste termijn zet ook "exact optellen".
    #[test]
    fn de_termijnen_zijn_gelijk_op_het_restant_na() {
        let amounts = split(Decimal::from(100_000), 12);
        assert_eq!(&amounts[..11], &[Decimal::from(8333); 11]);
        assert_eq!(amounts[11], Decimal::from(8337));
    }

    #[test]
    fn een_ritme_beschrijft_een_jaar() {
        assert_eq!(Schedule::Ineens.terms(), 1);
        assert_eq!(Schedule::Kwartaal.terms(), 4);
        assert_eq!(Schedule::Maand.terms(), 12);
        assert_eq!(Schedule::from_name("kwartaal"), Some(Schedule::Kwartaal));
        assert_eq!(Schedule::from_name("per_week"), None);
        assert_eq!(Schedule::Maand.name(), "maand");
    }

    /// Het bedrag komt uit de wet die het besluit uitvoert, en nergens anders
    /// vandaan.
    #[test]
    fn een_bedrag_dat_geen_uitkomst_van_het_besluit_is_wordt_geweigerd() {
        for amount in ["100000", "$standaardpremie"] {
            let err = valideer(&obligation(&format!(
                "soort: betaling\nbedrag: '{amount}'\nritme: ineens\ngrondslag: art. 1\n"
            )))
            .expect_err("een bedrag buiten de uitkomsten hoort te falen");
            assert!(
                matches!(err, SimulatorError::ObligationAmount { .. }),
                "verwachtte ObligationAmount voor '{amount}', kreeg {err}"
            );
        }
    }

    /// Een uitkomst van het artikel zélf mag ook, ook als het besluit haar niet
    /// publiceert: de verplichting staat in dat artikel, dus daar mag ze naar
    /// wijzen. Wat de uitvoering ervan oplevert, gaat mee als gevraagde uitkomst.
    #[test]
    fn een_bedrag_uit_het_artikel_zelf_mag_ook() {
        valideer(&obligation(
            "soort: betaling\nbedrag: $verzamelinkomen\nritme: ineens\ngrondslag: art. 1\n",
        ))
        .unwrap_or_else(|e| panic!("een uitkomst van het artikel hoort te mogen: {e}"));
    }

    /// Een soort die de opstelling niet kent hoort niet stil als betaling te
    /// eindigen.
    #[test]
    fn een_onbekende_soort_verplichting_wordt_geweigerd() {
        let err = valideer(&obligation(
            "soort: terugvordering\nbedrag: $hoogte_zorgtoeslag\nritme: ineens\ngrondslag: art. 1\n",
        ))
        .expect_err("een onbekende soort hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownObligationKind { .. }),
            "verwachtte UnknownObligationKind, kreeg {err}"
        );
    }

    #[test]
    fn een_letterlijk_ritme_dat_niet_bestaat_wordt_geweigerd() {
        let err = valideer(&obligation(
            "soort: betaling\nbedrag: $hoogte_zorgtoeslag\nritme: per_week\ngrondslag: art. 1\n",
        ))
        .expect_err("een ritme dat niet bestaat hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownSchedule { .. }),
            "verwachtte UnknownSchedule, kreeg {err}"
        );
    }

    /// Een `$instelling` valt hier niet na te kijken — dat doet de wereld — dus
    /// hier hoort ze door te komen.
    #[test]
    fn een_ritme_uit_een_instelling_komt_langs_de_toets_van_het_besluit() {
        valideer(&obligation(
            "soort: betaling\nbedrag: $hoogte_zorgtoeslag\nritme: $betalingsritme\ngrondslag: art. 1\n",
        ))
        .unwrap_or_else(|e| panic!("een verwijzing naar een instelling hoort te mogen: {e}"));
    }

    /// Staat er geen verwijzing in, dan ligt de datum bij het optuigen al vast en
    /// hoort een tekst die geen datum is daar te sneuvelen.
    #[test]
    fn een_vaste_vanaf_die_geen_datum_is_wordt_geweigerd() {
        let err = valideer(&betaling("vanaf: volgend jaar\n"))
            .expect_err("een `vanaf` die geen datum is hoort te falen");
        assert!(
            matches!(err, SimulatorError::MalformedObligationDate { .. }),
            "verwachtte MalformedObligationDate, kreeg {err}"
        );
    }

    #[test]
    fn een_vanaf_die_naar_een_onbekende_parameter_verwijst_wordt_geweigerd() {
        let err = valideer(&betaling("vanaf: '{toeslagjaar}-01-01'\n"))
            .expect_err("een verwijzing zonder gedocumenteerde parameter hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownReference { .. }),
            "verwachtte UnknownReference, kreeg {err}"
        );
    }

    /// Een sjabloon dat wél een datum oplevert, levert hem ook op het moment dat
    /// het ingevuld wordt.
    #[test]
    fn een_vanaf_met_een_parameter_levert_de_datum_op() {
        let obligation = betaling("vanaf: '{jaar}-02-01'\n");
        valideer(&obligation).unwrap_or_else(|e| panic!("dit sjabloon hoort te mogen: {e}"));

        let params = BTreeMap::from([("jaar".to_string(), Value::String("2027".to_string()))]);
        let start = obligation
            .start(
                "toeslagen",
                "toekenning",
                &params,
                parse_date("2026-12-15").unwrap_or_default(),
            )
            .unwrap_or_else(|e| panic!("de startdatum moet uit te rekenen zijn: {e}"));
        assert_eq!(
            start,
            ObligationStart::Op(parse_date("2027-02-01").unwrap_or_default())
        );
    }

    /// `vanaf: bekendmaking` levert geen datum maar de gebeurtenis waarop de
    /// termijn wacht — ook bij het optuigen, waar een sjabloon zonder
    /// verwijzingen anders een datum had moeten zijn.
    #[test]
    fn een_vanaf_bekendmaking_levert_de_gebeurtenis_op() {
        let obligation = betaling("vanaf: bekendmaking\nvervaldatum: uiterste_betaaldatum_4_87\n");
        valideer(&obligation)
            .unwrap_or_else(|e| panic!("de bekendmaking hoort een geldige `vanaf` te zijn: {e}"));

        let start = obligation
            .start(
                "toeslagen",
                "toekenning",
                &BTreeMap::new(),
                parse_date("2026-12-15").unwrap_or_default(),
            )
            .unwrap_or_else(|e| panic!("dit hoort geen fout te zijn: {e}"));
        assert_eq!(start, ObligationStart::Bekendmaking);
    }

    /// `vanaf: bekendmaking` zonder te zeggen welke uitkomst de vervaldag levert,
    /// valt bij het optuigen: het platform kent daar geen vaste naam voor.
    #[test]
    fn een_vanaf_bekendmaking_zonder_vervaldatum_wordt_geweigerd() {
        let err = valideer(&betaling("vanaf: bekendmaking\n"))
            .expect_err("zonder `vervaldatum` hoort dit te falen");
        assert!(
            matches!(err, SimulatorError::ObligationVervaldatum { .. })
                && err.to_string().contains("uiterste_betaaldatum_4_87"),
            "de melding hoort te noemen wat de stage wél levert, kreeg: {err}"
        );
    }

    /// Een `vervaldatum` die de bekendmaking niet oplevert, valt ook: anders
    /// valt de bekendmaking pas om bij de eerste zaak.
    #[test]
    fn een_vervaldatum_die_de_bekendmaking_niet_levert_wordt_geweigerd() {
        let err = valideer(&betaling(
            "vanaf: bekendmaking\nvervaldatum: uiterste_betaaldatum\n",
        ))
        .expect_err("een onbekende uitkomst hoort te falen");
        assert!(
            err.to_string().contains("'uiterste_betaaldatum'"),
            "de melding hoort de naam te noemen, kreeg: {err}"
        );
    }

    /// En een `vervaldatum` zonder `vanaf: bekendmaking` zou stil genegeerd
    /// worden.
    #[test]
    fn een_vervaldatum_zonder_bekendmaking_wordt_geweigerd() {
        let err = valideer(&betaling("vervaldatum: uiterste_betaaldatum_4_87\n"))
            .expect_err("een vervaldatum zonder de gebeurtenis hoort te falen");
        assert!(matches!(err, SimulatorError::ObligationVervaldatum { .. }));
    }

    /// Een wachtende verplichting reist door het gram heen: wat erin gaat, komt
    /// er hetzelfde weer uit. Zonder die eigenschap zou de bekendmaking een
    /// andere termijn inroosteren dan het besluit beloofde.
    #[test]
    fn een_wachtende_verplichting_overleeft_het_gram() {
        let wachtend = WachtendeVerplichting {
            soort: ObligationKind::Betaling,
            schuldenaar: "Uitvoerder".to_string(),
            schuldeiser: "999993653".to_string(),
            betaler: Some("uitvoerder".to_string()),
            bedrag: Decimal::from(42000),
            schedule: Schedule::Kwartaal,
            eerste_volgnummer: 2,
            termijnen: 5,
            grondslag: "art. 1 jo. art. 4:87".to_string(),
            herkomst: ObligationOrigin {
                regulation: "test_bekendmaking".to_string(),
                valid_from: Some("2024-01-01".to_string()),
                article: "1".to_string(),
            },
            vervaldatum_uit: "uiterste_betaaldatum_4_87".to_string(),
        };
        let teruggelezen = WachtendeVerplichting::from_value(&wachtend.as_value())
            .unwrap_or_else(|| panic!("wat het gram draagt, hoort leesbaar te zijn"));
        assert_eq!(teruggelezen, wachtend);
    }

    /// En de termijnen die eruit volgen sluiten aan op het schema van het
    /// besluit: ze beginnen bij het volgnummer dat daar gereserveerd is, en het
    /// ritme bepaalt de dagen.
    #[test]
    fn een_wachtende_verplichting_levert_termijnen_vanaf_een_datum() {
        let wachtend = WachtendeVerplichting {
            soort: ObligationKind::Betaling,
            schuldenaar: "Uitvoerder".to_string(),
            schuldeiser: "999993653".to_string(),
            betaler: None,
            bedrag: Decimal::from(400),
            schedule: Schedule::Kwartaal,
            eerste_volgnummer: 2,
            termijnen: 5,
            grondslag: "art. 1".to_string(),
            herkomst: ObligationOrigin {
                regulation: "test_bekendmaking".to_string(),
                valid_from: None,
                article: "1".to_string(),
            },
            vervaldatum_uit: "uiterste_betaaldatum_4_87".to_string(),
        };
        let zaak = BesluitGram {
            cell: "uitvoerder",
            besluit: "toekenning",
            zaakkenmerk: "tegemoetkoming/999993653",
            op_moment: parse_date("2024-03-01").unwrap_or_default(),
            plek: 3,
        };
        let termijnen = wachtend
            .termijnen_vanaf(
                parse_date("2024-05-27").unwrap_or_default(),
                parse_date("2024-05-01").unwrap_or_default(),
                &zaak,
            )
            .unwrap_or_else(|e| panic!("de termijnen horen uit te rekenen te zijn: {e}"));
        let dagen: Vec<String> = termijnen
            .iter()
            .map(|due| due.vervaldatum.to_string())
            .collect();
        assert_eq!(
            dagen,
            vec!["2024-05-27", "2024-08-27", "2024-11-27", "2025-02-27"]
        );
        let nummers: Vec<i64> = termijnen.iter().map(|due| due.volgnummer).collect();
        assert_eq!(nummers, vec![2, 3, 4, 5]);
        assert!(
            termijnen.iter().all(|due| due.besluit_gram == 3),
            "elke termijn hoort naar het besluit-gram te wijzen en niet naar de bekendmaking"
        );
    }

    /// Een termijn die vóór de bekendmaking zou vervallen, wordt op de dag van
    /// de bekendmaking ingehaald; de rest van het schema blijft staan, met
    /// dezelfde volgnummers en bedragen.
    #[test]
    fn een_termijn_voor_de_bekendmaking_wordt_op_die_dag_ingehaald() {
        let wachtend = WachtendeVerplichting {
            soort: ObligationKind::Betaling,
            schuldenaar: "Uitvoerder".to_string(),
            schuldeiser: "999993653".to_string(),
            betaler: None,
            bedrag: Decimal::from(400),
            schedule: Schedule::Kwartaal,
            eerste_volgnummer: 1,
            termijnen: 4,
            grondslag: "art. 1".to_string(),
            herkomst: ObligationOrigin {
                regulation: "test_bekendmaking".to_string(),
                valid_from: None,
                article: "1".to_string(),
            },
            vervaldatum_uit: "uiterste_betaaldatum_4_87".to_string(),
        };
        let zaak = BesluitGram {
            cell: "uitvoerder",
            besluit: "toekenning",
            zaakkenmerk: "tegemoetkoming/999993653",
            op_moment: parse_date("2024-01-01").unwrap_or_default(),
            plek: 0,
        };
        let termijnen = wachtend
            .termijnen_vanaf(
                parse_date("2024-01-01").unwrap_or_default(),
                parse_date("2024-05-15").unwrap_or_default(),
                &zaak,
            )
            .unwrap_or_else(|e| panic!("de termijnen horen uit te rekenen te zijn: {e}"));
        let dagen: Vec<(String, Option<String>)> = termijnen
            .iter()
            .map(|due| {
                (
                    due.vervaldatum.to_string(),
                    due.oorspronkelijke_vervaldatum.map(|dag| dag.to_string()),
                )
            })
            .collect();
        assert_eq!(
            dagen,
            vec![
                ("2024-05-15".to_string(), Some("2024-01-01".to_string())),
                ("2024-05-15".to_string(), Some("2024-04-01".to_string())),
                ("2024-07-01".to_string(), None),
                ("2024-10-01".to_string(), None),
            ]
        );
        let nummers: Vec<i64> = termijnen.iter().map(|due| due.volgnummer).collect();
        assert_eq!(nummers, vec![1, 2, 3, 4]);
        assert!(
            termijnen[0]
                .describe()
                .contains("ingehaald (oorspronkelijk 2024-01-01)"),
            "de regel hoort te zeggen dat de termijn ingehaald wordt: {}",
            termijnen[0].describe()
        );
        let Value::Object(velden) = termijnen[0].as_value() else {
            panic!("een termijn is een object");
        };
        assert_eq!(
            velden
                .get(OORSPRONKELIJKE_VERVALDATUM)
                .and_then(Value::as_str),
            Some("2024-01-01")
        );
        let Value::Object(velden) = termijnen[2].as_value() else {
            panic!("een termijn is een object");
        };
        assert!(
            !velden.contains_key(OORSPRONKELIJKE_VERVALDATUM),
            "een termijn op haar eigen dag draagt geen oorspronkelijke vervaldatum"
        );
    }

    #[test]
    fn een_zaakkenmerk_dat_naar_een_onbekende_parameter_verwijst_wordt_geweigerd() {
        let err = definition("zorgtoeslag/{burgerservicenummer}")
            .validate_zaakkenmerk("toeslagen")
            .expect_err("een verwijzing zonder gedocumenteerde parameter hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownReference { .. }),
            "verwachtte UnknownReference, kreeg {err}"
        );
    }
}
