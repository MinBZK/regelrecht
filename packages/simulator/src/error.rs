//! Fouten van de simulator.
//!
//! De teksten zijn Nederlands, net als de rest van de gebruikersgerichte
//! uitvoer in deze repo; identifiers blijven Engels.

use std::fmt;
use std::path::PathBuf;

/// Waar een melding over gaat: een gepubliceerde lexostatus of een
/// besluit-definitie.
///
/// De twee delen hun controles — gedocumenteerde parameters, een eigen
/// regeling, bestaande uitkomsten, bestaande kroniekvelden — en dus ook de
/// fouten die daaruit komen. Wat er per melding bij hoort te staan is welke van
/// de twee de configuratie bedoelde, en dat is precies wat dit type draagt. Een
/// tweede set varianten met dezelfde tekst zou uiteen gaan lopen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    /// Een gepubliceerde lexostatus: een reductie die bevraagd wordt.
    Lexostatus,
    /// Een besluit-definitie: een wetsuitvoering die vastgelegd wordt.
    Besluit,
    /// Een actie van een actor op de tijdlijn.
    Actie,
    /// Een termijn die waarschuwt als een feit ontbreekt.
    Termijn,
    /// Een fictieve aanvrager in het portaal.
    Persona,
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Lexostatus => "lexostatus",
            Self::Besluit => "besluit",
            Self::Actie => "actie",
            Self::Termijn => "termijn",
            Self::Persona => "persona",
        })
    }
}

/// Alles wat er mis kan gaan bij het optuigen of bevragen van een cel.
#[derive(Debug, thiserror::Error)]
pub enum SimulatorError {
    /// Een consument vroeg een lexostatus op die de cel niet publiceert.
    ///
    /// Dit is het enige "niet gevonden" dat een consument kan zien: de cel
    /// biedt uitsluitend gepubliceerde namen aan (RFC-022 §4.1).
    #[error("cel '{cell}' publiceert geen lexostatus '{requested}' (wel: {published})")]
    UnknownLexostatus {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// De gevraagde naam.
        requested: String,
        /// Komma-gescheiden lijst van namen die de cel wél publiceert.
        published: String,
    },

    /// Een lezer wees een kroniekstroom aan die de cel niet houdt.
    ///
    /// Naast [`SimulatorError::UnknownStream`] en niet in plaats daarvan: die
    /// gaat over een *definitie* die naar een stroom verwijst en blijkt bij het
    /// optuigen. Dit is een vraag van buiten naar wat er in een kroniek ligt, en
    /// ze noemt dus geen definitie die fout zou zijn — er is er geen.
    #[error("cel '{cell}' houdt geen kroniekstroom '{stream}' (wel: {known})")]
    UnknownChronicle {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// De gevraagde stroomnaam.
        stream: String,
        /// Komma-gescheiden lijst van stromen die de cel wél houdt.
        known: String,
    },

    /// Een lezer wees een plek in een kroniek aan waar niets ligt.
    ///
    /// De plek is de volgorde van vastlegging, en een kroniek groeit uitsluitend
    /// achteraan: wat er nu niet is, was er ook nooit. De melding noemt daarom
    /// hoeveel er wél liggen, zodat een lezer ziet of hij te ver keek of naar een
    /// stroom die nog leeg is.
    #[error(
        "cel '{cell}': kroniekstroom '{stream}' heeft geen gram op plek {index} \
         (er liggen er {count})"
    )]
    UnknownGram {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// De stroom waarin gekeken is.
        stream: String,
        /// De gevraagde plek, geteld vanaf nul.
        index: usize,
        /// Hoeveel grammen er in die stroom liggen.
        count: usize,
    },

    /// Het receipt van een gram is opgevraagd, maar dit gram draagt er geen.
    ///
    /// Alleen een decretogram uit het besluit-pad draagt een RFC-013 Execution
    /// Receipt: het *is* de uitvoering. Een executogram is een feit dat de cel
    /// overkwam en een bron-cel kan zelf iets vaststellen zonder engine; in
    /// beide gevallen is er nooit een uitvoering geweest, en dan is "geen
    /// receipt" het juiste antwoord en niet een leeg receipt.
    #[error(
        "cel '{cell}': gram '{name}' op plek {index} van kroniekstroom '{stream}' is geen \
         decretogram uit het besluit-pad en draagt dus geen uitvoeringsreceipt"
    )]
    GramWithoutReceipt {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// De stroom waarin het gram ligt.
        stream: String,
        /// De plek van het gram in die stroom.
        index: usize,
        /// Hoe het gram heet.
        name: String,
    },

    /// Een statusindicator vult de parameters van haar lexostatus niet precies.
    ///
    /// Bij het optuigen en niet bij de eerste meting: een indicator die zijn
    /// lexostatus niet kan bevragen, zou anders stil nooit een verandering
    /// opleveren — en dan is niet te zien of er niets gebeurde of dat het
    /// wereldbestand een typfout draagt.
    #[error(
        "cel '{cell}': statusindicator op lexostatus '{lexostatus}' vult parameters \
         [{given}], maar die lexostatus vraagt [{expected}]"
    )]
    StatusIndicatorParams {
        /// De cel waarin de indicator staat.
        cell: String,
        /// De lexostatus die de stand zou dragen.
        lexostatus: String,
        /// Komma-gescheiden lijst van wat de indicator vult.
        given: String,
        /// Komma-gescheiden lijst van wat de lexostatus vraagt.
        expected: String,
    },

    /// Een gedocumenteerde parameter ontbreekt in de vraag.
    #[error("{subject} '{cell}.{name}' vereist parameter '{parameter}'")]
    MissingParameter {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// Of dit over een lexostatus of over een besluit gaat.
        subject: Subject,
        /// De gevraagde lexostatus of het uitgevoerde besluit.
        name: String,
        /// De ontbrekende parameter.
        parameter: String,
    },

    /// De vraag bevat een parameter die de definitie niet documenteert.
    #[error(
        "{subject} '{cell}.{name}' kent geen parameter '{parameter}' \
         (gedocumenteerd: {documented})"
    )]
    UndocumentedParameter {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// Of dit over een lexostatus of over een besluit gaat.
        subject: Subject,
        /// De gevraagde lexostatus of het uitgevoerde besluit.
        name: String,
        /// De onbekende parameter.
        parameter: String,
        /// Komma-gescheiden lijst van gedocumenteerde parameters.
        documented: String,
    },

    /// Een parameter heeft een ander type dan gedocumenteerd.
    #[error(
        "parameter '{parameter}' van {subject} '{cell}.{name}' is {actual}, \
         verwacht {expected}"
    )]
    ParameterType {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// Of dit over een lexostatus of over een besluit gaat.
        subject: Subject,
        /// De gevraagde lexostatus of het uitgevoerde besluit.
        name: String,
        /// De parameter met het verkeerde type.
        parameter: String,
        /// Het gedocumenteerde type.
        expected: &'static str,
        /// Het aangeboden type.
        actual: &'static str,
    },

    /// Een parameter van het type `date` draagt geen ISO-datum.
    ///
    /// Een eigen melding naast [`Self::ParameterType`], want het bezwaar is een
    /// ander: de waarde is wél tekst, ze is alleen geen datum. "verwacht date,
    /// kreeg string" zou hier staan en zou de invuller die `01-12-2026` typte
    /// niets zeggen; deze melding noemt de notatie die wél gelezen wordt.
    #[error(
        "parameter '{parameter}' van {subject} '{cell}.{name}' is geen datum: '{value}' \
         (verwacht jjjj-mm-dd)"
    )]
    ParameterDate {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// Of dit over een lexostatus, een besluit of een actie gaat.
        subject: Subject,
        /// De gevraagde lexostatus, het uitgevoerde besluit of de actie.
        name: String,
        /// De parameter met de onleesbare datum.
        parameter: String,
        /// Wat er stond.
        value: String,
    },

    /// De definitie verwijst met `$naam` of `{naam}` naar een
    /// niet-gedocumenteerde parameter.
    #[error(
        "cel '{cell}': {subject} '{name}' verwijst naar parameter '{reference}', \
         maar die is niet gedocumenteerd"
    )]
    UnknownReference {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Of dit over een lexostatus of over een besluit gaat.
        subject: Subject,
        /// De definitie met de kapotte verwijzing.
        name: String,
        /// De naam waarnaar verwezen wordt.
        reference: String,
    },

    /// Twee lexostatussen met dezelfde naam in één cel.
    #[error("cel '{cell}' definieert lexostatus '{name}' twee keer")]
    DuplicateLexostatus {
        /// Cel waarin het dubbel staat.
        cell: String,
        /// De dubbele naam.
        name: String,
    },

    /// Een definitie grijpt naar een regeling die deze cel niet zelf geladen heeft.
    ///
    /// Een reductie raakt uitsluitend de eigen feiten van de cel; combineren over
    /// cellen heen is synthese en hoort bij een consument (RFC-022 §4.1). Een
    /// besluit voert een eigen regeling uit, om dezelfde reden: de cel besluit
    /// op haar eigen recht.
    #[error(
        "cel '{cell}': {subject} '{name}' gebruikt regeling '{regulation}', \
         die deze cel niet zelf geladen heeft"
    )]
    ForeignRegulation {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Of dit over een lexostatus of over een besluit gaat.
        subject: Subject,
        /// De definitie met de vreemde regeling.
        name: String,
        /// De regeling waarnaar verwezen wordt.
        regulation: String,
    },

    /// Een definitie publiceert een uitkomst die haar bron niet kent.
    ///
    /// Wat een cel publiceert is een belofte aan de consument; een naam die
    /// geen enkele geladen versie van de regeling oplevert — of die in geen
    /// enkele vastlegging van de kroniekstroom voorkomt — kan die belofte niet
    /// waarmaken. Dat blijkt bij het optuigen, net als bij
    /// [`SimulatorError::ForeignRegulation`], en niet pas bij de eerste vraag.
    #[error(
        "cel '{cell}': {subject} '{name}' publiceert uitkomst '{output}', \
         maar {origin} kent die niet (wel: {known})"
    )]
    UnknownOutput {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Of dit over een lexostatus of over een besluit gaat.
        subject: Subject,
        /// De definitie met de onbekende uitkomst.
        name: String,
        /// Waar de uitkomst uit had moeten komen: `regeling '…'` of
        /// `kroniekstroom '…'`. Niet `source`: dat veld zou thiserror als de
        /// onderliggende fout lezen.
        origin: String,
        /// De uitkomstnaam die niet bestaat.
        output: String,
        /// Komma-gescheiden lijst van namen die de bron wél kent.
        known: String,
    },

    /// Een definitie verwijst naar een kroniekstroom die de cel niet houdt.
    ///
    /// Een reductie raakt uitsluitend de eigen feiten van de cel, en een besluit
    /// leest zijn inputs uit die eigen kronieken. Een onbekende stroomnaam is de
    /// kroniek-tegenhanger van [`SimulatorError::ForeignRegulation`] en blijkt op
    /// hetzelfde moment.
    #[error(
        "cel '{cell}': {subject} '{name}' verwijst naar kroniekstroom \
         '{stream}', die deze cel niet houdt (wel: {known})"
    )]
    UnknownStream {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Of dit over een lexostatus of over een besluit gaat.
        subject: Subject,
        /// De definitie met de onbekende stroom.
        name: String,
        /// De stroomnaam die niet bestaat.
        stream: String,
        /// Komma-gescheiden lijst van stromen die de cel wél houdt.
        known: String,
    },

    /// Een kroniekfilter zegt niet wat het publiceert.
    ///
    /// Bij de wetsvorm is er altijd één uitkomst die de lexostatus *is*; een
    /// kroniekfilter heeft die niet. Zonder `outputs` zou de cel dus haar hele
    /// vastlegging naar buiten geven zonder iets beloofd te hebben.
    #[error(
        "cel '{cell}': kroniekfilter van '{lexostatus}' op '{stream}' moet in `outputs` \
         noemen wat het publiceert; een kroniekfilter heeft geen wetsuitkomst \
         die dat bepaalt"
    )]
    ChronicleWithoutOutputs {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus zonder `outputs`.
        lexostatus: String,
        /// De stroom waarover gefilterd wordt.
        stream: String,
    },

    /// De openstaandvorm noemt `outputs`, terwijl ze haar eigen uitkomsten maakt.
    ///
    /// Anders dan bij een kroniekfilter is er hier niets te kiezen: de drie
    /// bedragen en de termijnenlijst zijn wat deze vorm oplevert, en ze horen bij
    /// elkaar — zonder de lijst is het bedrag niet na te rekenen. Wat een
    /// definitie er zelf bij zou noemen, zou nooit in het antwoord komen.
    #[error(
        "cel '{cell}': '{lexostatus}' is een openstaandvorm en publiceert altijd \
         {published}; laat `outputs` weg in plaats van {outputs} te beloven"
    )]
    OpenstaandWithOutputs {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus met de overbodige `outputs`.
        lexostatus: String,
        /// Wat de definitie noemde.
        outputs: String,
        /// Wat de vorm werkelijk publiceert.
        published: String,
    },

    /// Een som publiceert iets anders dan het veld waarover ze sommeert.
    ///
    /// Een som levert precies één waarde op: het totaal van dat ene veld. Een
    /// `outputs` met iets anders erin belooft een uitkomst die nooit in het
    /// antwoord komt, en die belofte hoort bij het optuigen te sneuvelen.
    #[error(
        "cel '{cell}': '{lexostatus}' sommeert veld '{field}', dus `outputs` hoort \
         precies dat veld te noemen (nu: {outputs})"
    )]
    SumOutputMismatch {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus met de som.
        lexostatus: String,
        /// Het veld waarover gesommeerd wordt.
        field: String,
        /// Komma-gescheiden lijst van wat er nu in `outputs` staat.
        outputs: String,
    },

    /// Een som stuit op een vastlegging zonder getal in het veld.
    ///
    /// Een som die zo'n vastlegging overslaat, valt stil te laag uit, en dan is
    /// "betaald tot nu toe" een getal dat er goed uitziet en niet klopt. Een
    /// stroom die zich niet laat sommeren is een fout in de wereld, niet een
    /// reden om te gokken.
    #[error(
        "cel '{cell}': '{lexostatus}' sommeert '{field}', maar de vastlegging van \
         {op_moment} heeft daar {found}"
    )]
    SumOfNonNumber {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// De lexostatus met de som.
        lexostatus: String,
        /// Stroom en veld die een getal moesten leveren, als `stroom.veld`.
        field: String,
        /// Het moment van de vastlegging die niet meekon.
        op_moment: String,
        /// Wat er in plaats van een getal stond.
        found: String,
    },

    /// Een definitie sleutelt, filtert of leest op een veld dat de stroom niet kent.
    ///
    /// Zo'n filter zou stil "niets vastgesteld" antwoorden op elke vraag, en dat
    /// is niet te onderscheiden van een leeg verleden; een besluit zou een input
    /// missen zonder dat iemand de typfout ziet.
    #[error(
        "cel '{cell}': {subject} '{name}' gebruikt veld '{field}', \
         maar kroniekstroom '{stream}' kent dat niet (wel: {known})"
    )]
    UnknownFilterField {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Of dit over een lexostatus of over een besluit gaat.
        subject: Subject,
        /// De definitie met het onbekende veld.
        name: String,
        /// De stroom waarover gefilterd wordt.
        stream: String,
        /// Het veld dat de stroom niet kent.
        field: String,
        /// Komma-gescheiden lijst van velden die de stroom wél kent.
        known: String,
    },

    /// Een `where`-voorwaarde vergelijkt met een `$`-verwijzing.
    ///
    /// `where` kent alleen letterlijke waarden; de `$naam`-vorm van de wetsvorm
    /// betekent hier niets. Zonder deze controle zou zo'n filter de tekst
    /// `$naam` zoeken en op elke vraag "niets vastgesteld" antwoorden, wat niet
    /// te onderscheiden is van een leeg verleden.
    #[error(
        "cel '{cell}': `where` van '{lexostatus}' vergelijkt veld '{field}' met \
         '${reference}', maar `where` vergelijkt met letterlijke waarden; de enige \
         plek waar een kroniekfilter een parameter gebruikt, is `key`"
    )]
    FilterValueReference {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus met de kapotte voorwaarde.
        lexostatus: String,
        /// Het veld waarop de voorwaarde staat.
        field: String,
        /// De naam waarnaar verwezen werd.
        reference: String,
    },

    /// De sleutel van een kroniekfilter is geen gedocumenteerde parameter.
    ///
    /// De consument levert de sleutelwaarde aan; staat de sleutel niet in
    /// `inputs`, dan is er niets dat hem aanlevert en gaat de vraag over geen
    /// enkel onderwerp.
    #[error(
        "cel '{cell}': kroniekfilter van '{lexostatus}' sleutelt op '{key}', maar dat \
         is geen gedocumenteerde parameter (gedocumenteerd: {documented})"
    )]
    ChronicleKeyWithoutParameter {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus met de onbereikbare sleutel.
        lexostatus: String,
        /// Het sleutelveld van het filter.
        key: String,
        /// Komma-gescheiden lijst van gedocumenteerde parameters.
        documented: String,
    },

    /// Er wordt een besluit gevraagd dat deze cel niet kent.
    ///
    /// De tegenhanger van [`SimulatorError::UnknownLexostatus`]: een cel voert
    /// uitsluitend besluiten uit die als definitie in haar configuratie staan.
    /// Een besluit dat nergens gedefinieerd is, is geen besluit.
    #[error("cel '{cell}' kent geen besluit '{requested}' (wel: {defined})")]
    UnknownBesluit {
        /// De cel waaraan het besluit gevraagd werd.
        cell: String,
        /// De gevraagde naam.
        requested: String,
        /// Komma-gescheiden lijst van besluiten die de cel wél kent.
        defined: String,
    },

    /// Een besluit-definitie stuurt op een uitkomst die geen beschikking is.
    ///
    /// Een decretogram is een engine-uitkomst met `legal_character: BESCHIKKING`
    /// (RFC-022 §1.2). Een toets of een waardebepaling als besluit vastleggen zou
    /// een gram in de stroom met beschikkingen leggen dat geen beschikking is.
    #[error(
        "cel '{cell}': besluit '{besluit}' stuurt op uitkomst '{output}' van regeling \
         '{regulation}', maar die is geen beschikking (rechtskarakter: {found}); een \
         decretogram is een uitkomst met `legal_character: BESCHIKKING` (RFC-022 §1.2)"
    )]
    BesluitNotABeschikking {
        /// De cel waarin de definitie staat.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// De regeling die het besluit uitvoert, bij `$id`.
        regulation: String,
        /// De aansturende uitkomst.
        output: String,
        /// Wat de geladen versies er wél van maken.
        found: String,
    },

    /// Een besluit-definitie declareert zelf wanneer ze afwijst.
    ///
    /// Dat hoort in het lexogram: wanneer een besluit een afwijzing is, hangt aan
    /// de uitkomst die het artikel voortbrengt en geldt voor elke cel die dat
    /// artikel uitvoert. Zou een wereldbestand het mogen zetten, dan konden twee
    /// uitvoerders dezelfde wet verschillend laten weigeren zonder dat er aan de
    /// wet iets te zien was.
    #[error(
        "cel '{cell}': besluit '{besluit}' declareert `afwijzing_wanneer`, maar dat hoort \
         in de regeling: zet het blok op het artikel van regeling '{regulation}' dat \
         uitkomst '{output}' voortbrengt, onder `produces.extensions.chronolex.\
         afwijzing_wanneer`"
    )]
    AfwijzingWanneerInWereldbestand {
        /// De cel waarin de definitie staat.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// De regeling die het besluit uitvoert, bij `$id`.
        regulation: String,
        /// De aansturende uitkomst, en daarmee het artikel waar het blok hoort.
        output: String,
    },

    /// Het `afwijzing_wanneer`-blok in een regeling heeft niet de vorm van een
    /// voorwaarde.
    ///
    /// Het blok staat in een **wet**, dus wie het schrijft is niet dezelfde als
    /// wie het leest. Een blok dat stil als "geen voorwaarde" zou eindigen, zet
    /// de weigering uit zonder dat er iets te zien is.
    #[error(
        "cel '{cell}': besluit '{besluit}' voert regeling '{regulation}' uit, en het \
         artikel achter uitkomst '{output}' declareert een \
         `produces.extensions.chronolex.afwijzing_wanneer` die niet te lezen is: {reason}"
    )]
    MalformedAfwijzingWanneer {
        /// De cel waarin de definitie staat.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// De regeling die het besluit uitvoert, bij `$id`.
        regulation: String,
        /// De aansturende uitkomst, en daarmee het artikel met het blok.
        output: String,
        /// Wat er aan het blok niet klopt.
        reason: String,
    },

    /// Een `afwijzing_wanneer` noemt een uitkomst die geen ja-of-nee is.
    ///
    /// Een voorwaarde vergelijkt met `true` of `false`; op een bedrag of een
    /// datum raakt ze nooit vervuld. Dan staat er een afwijzing in de regeling
    /// die nooit afwijst, en dat hoort te blijken bij het optuigen van de cel en
    /// niet bij de eerste aanvrager die geweigerd had moeten worden.
    #[error(
        "cel '{cell}': besluit '{besluit}' wijst volgens regeling '{regulation}' af bij \
         uitkomst '{output}', maar die is geen ja-of-nee (type: {found}); een \
         afwijzingsvoorwaarde vergelijkt met `true` of `false`"
    )]
    AfwijzingsvoorwaardeNotBoolean {
        /// De cel waarin de definitie staat.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// De regeling die het besluit uitvoert, bij `$id`.
        regulation: String,
        /// De uitkomst waarop de voorwaarde slaat.
        output: String,
        /// De typen die de geladen versies eraan geven.
        found: String,
    },

    /// Een cel-id in de wereld is ook de `$id` van een regeling die een cel laadt.
    ///
    /// RFC-022 §4.2 maakt daar een laadfout van, onvoorwaardelijk: een vraag aan
    /// die cel zou door de gelijknamige regeling beantwoord worden, zonder dat
    /// iemand het ziet. De engine weigert het al per cel; de wereld is de enige
    /// die álle cellen en álle geladen regelingen naast elkaar heeft, en toetst
    /// het daarom ook over de cellen heen.
    #[error(
        "cel-id '{cell}' is ook de $id van een regeling die cel '{loaded_by}' laadt; \
         een vraag aan die cel zou door die regeling beantwoord worden (RFC-022 §4.2). \
         Hernoem de cel"
    )]
    CellIdShadowsRegulation {
        /// Het cel-id dat een regeling overschaduwt.
        cell: String,
        /// De cel die de gelijknamige regeling laadt.
        loaded_by: String,
    },

    /// De cel die wil besluiten, is niet het bevoegd gezag van de regeling.
    ///
    /// De wet bepaalt wie mag besluiten; de cel beweert wie zij is. Lopen die
    /// twee uiteen, dan is er geen besluit te nemen en wordt er niets
    /// vastgelegd — ook geen gram met een aantekening erbij, want een besluit
    /// van een onbevoegde is geen besluit dat later nog goed te keuren valt.
    #[error(
        "cel '{cell}' beweert '{identity}' te zijn, maar regeling '{regulation}' wijst \
         '{authority}' aan als bevoegd gezag (besluit '{besluit}')"
    )]
    NotCompetentAuthority {
        /// Het cel-id van de cel die wilde besluiten.
        cell: String,
        /// De besluit-definitie die zij wilde uitvoeren.
        besluit: String,
        /// De naam waaronder die cel zich uitgeeft.
        identity: String,
        /// De regeling die het besluit uitvoert, bij `$id`.
        regulation: String,
        /// Het bevoegd gezag dat die regeling aanwijst.
        authority: String,
    },

    /// Twee besluit-definities met dezelfde naam in één cel.
    #[error("cel '{cell}' definieert besluit '{name}' twee keer")]
    DuplicateBesluit {
        /// Cel waarin het dubbel staat.
        cell: String,
        /// De dubbele naam.
        name: String,
    },

    /// De configuratie declareert een kroniekstroom met een naam die het
    /// platform zelf gebruikt.
    ///
    /// De stroom voor decretogrammen hoort bij het besluit-pad en wordt
    /// automatisch aangemaakt zodra een cel besluit-definities heeft. Wie haar
    /// zelf declareert, zou een tweede stroom met dezelfde naam krijgen (waar de
    /// ene de andere stil schaduwt) of vastleggingen tussen de decretogrammen
    /// zetten die geen besluit zijn.
    #[error(
        "cel '{cell}': kroniekstroom '{stream}' is voorbehouden aan het besluit-pad \
         en wordt automatisch aangemaakt zodra de cel besluit-definities heeft; \
         kies een andere naam"
    )]
    ReservedStream {
        /// Cel waarin de stroom gedeclareerd staat.
        cell: String,
        /// De voorbehouden stroomnaam.
        stream: String,
    },

    /// Een vastlegging van buiten het besluit-pad mikt op de stroom met
    /// decretogrammen.
    ///
    /// Dat de configuratie die stroom niet mag declareren
    /// ([`SimulatorError::ReservedStream`]) is niet genoeg: zodra een cel
    /// besluit-definities heeft, bestaat de stroom, en een `fixture` zou er een
    /// gram in kunnen zetten dat nooit langs een engine kwam — zonder receipt en
    /// met een `intake` naar keuze. Een reductie erover zou dat niet van een
    /// besluit kunnen onderscheiden.
    #[error(
        "cel '{cell}': kroniekstroom '{stream}' is voorbehouden aan het besluit-pad; \
         vastlegging '{name}' van {op_moment} hoort daar niet in — een decretogram \
         ontstaat door te besluiten, niet door het op te schrijven"
    )]
    ReservedStreamRecording {
        /// De cel waarin vastgelegd zou worden.
        cell: String,
        /// De voorbehouden stroomnaam.
        stream: String,
        /// De naam van de vastlegging die geweigerd wordt.
        name: String,
        /// Het moment van die vastlegging.
        op_moment: String,
    },

    /// Een besluit leest een input uit een stroom waarvan het sleutelveld geen
    /// gedocumenteerde parameter van dat besluit is.
    ///
    /// Het besluit levert de sleutelwaarde aan; zonder parameter is er niets dat
    /// haar aanlevert en gaat de vraag naar de kroniek over geen enkel onderwerp.
    #[error(
        "cel '{cell}': besluit '{besluit}' leest uit kroniekstroom '{stream}', \
         die sleutelt op '{key}'; dat moet een gedocumenteerde parameter van het \
         besluit zijn (gedocumenteerd: {documented})"
    )]
    BesluitStreamKeyWithoutParameter {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de onbereikbare sleutel.
        besluit: String,
        /// De stroom waaruit gelezen wordt.
        stream: String,
        /// Het sleutelveld van die stroom.
        key: String,
        /// Komma-gescheiden lijst van gedocumenteerde parameters.
        documented: String,
    },

    /// Een besluit geeft de engine een input die de regeling niet declareert.
    ///
    /// Dan wordt de waarde opgehaald, in het decretogram vastgelegd en door de
    /// engine genegeerd: een besluit dat zegt te leunen op een feit dat niets
    /// deed. Dat is een typfout, en die hoort bij het optuigen te blijken.
    #[error(
        "cel '{cell}': besluit '{besluit}' levert input '{input}', maar regeling \
         '{regulation}' kent geen parameter of input met die naam (wel: {known})"
    )]
    UnknownRegulationInput {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de onbekende input.
        besluit: String,
        /// De inputnaam die de regeling niet kent.
        input: String,
        /// De regeling die het besluit uitvoert.
        regulation: String,
        /// Komma-gescheiden lijst van namen die de regeling wél kent.
        known: String,
    },

    /// Een besluit wil een input uit de stroom met decretogrammen halen.
    ///
    /// Dan zou het nieuwe besluit leunen op een veld van een eerder besluit, en
    /// is niet meer te zeggen of er gerekend of overgeschreven is — dezelfde
    /// schaduwboekhouding die de stroom al buiten de databronnen van de engine
    /// houdt, langs de andere weg. Terugzien doe je met een reductie over deze
    /// stroom; leunen op een eerder besluit is een eigen stap en geen input.
    #[error(
        "cel '{cell}': besluit '{besluit}' haalt input '{input}' uit kroniekstroom \
         '{stream}' (veld '{field}'), maar daar liggen besluiten en geen feiten; \
         een besluit leest geen besluit"
    )]
    DecretogramAsBesluitInput {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit dat uit de verkeerde stroom leest.
        besluit: String,
        /// De input die eruit zou komen.
        input: String,
        /// De voorbehouden stroomnaam.
        stream: String,
        /// Het veld dat gelezen zou worden.
        field: String,
    },

    /// Een besluit leest terug uit een besluit dat deze cel niet kent.
    ///
    /// De tegenhanger van [`SimulatorError::DecretogramAsBesluitInput`] aan de
    /// goede kant: teruglezen mag, maar dan wel uit een besluit dat er is. Een
    /// naam die nergens op uitkomt zou bij elke zaak "geen eerder besluit"
    /// opleveren, en dat is niet te onderscheiden van een zaak die nog geen
    /// geschiedenis heeft.
    #[error(
        "cel '{cell}': besluit '{besluit}' leest terug uit besluit '{earlier}', \
         maar die cel kent dat besluit niet (wel: {known})"
    )]
    UnknownEarlierBesluit {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit dat terugleest.
        besluit: String,
        /// De besluitnaam waaruit gelezen zou worden.
        earlier: String,
        /// Komma-gescheiden lijst van besluiten die de cel wél kent.
        known: String,
    },

    /// Een besluit leest een veld terug dat een gram van dat eerdere besluit
    /// niet draagt.
    ///
    /// Bij het optuigen en niet bij het besluit: welke velden een gram draagt —
    /// de uitkomsten, de vaste velden en de inputs — staat vast zodra de
    /// definities er zijn, en een typfout hoort niet te wachten tot er een zaak
    /// is om op stuk te lopen.
    #[error(
        "cel '{cell}': besluit '{besluit}' leest veld '{field}' uit eerder besluit \
         '{earlier}', maar een gram van dat besluit draagt dat veld niet (wel: {known})"
    )]
    UnknownEarlierBesluitField {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit dat terugleest.
        besluit: String,
        /// Het besluit waaruit gelezen wordt.
        earlier: String,
        /// Het veld dat gelezen zou worden.
        field: String,
        /// Komma-gescheiden lijst van velden die zo'n gram wél draagt.
        known: String,
    },

    /// Een besluit leest een veld terug dat een gram van dat eerdere besluit
    /// **twee keer** draagt: als uitkomst of vast veld, én als input.
    ///
    /// Dan wijst één naam twee waarden aan, en welke van de twee er gepakt wordt
    /// is een leesregel die niemand bij het schrijven voor ogen had. Het beeld
    /// van de wereld kiest bij zo'n botsing de input (daar is de herkomst
    /// rijker), een leesregel hier zou eerder het veld van het gram zelf pakken —
    /// twee antwoorden op dezelfde vraag. Bij het optuigen weigeren dus: hernoem
    /// de input, of lees een veld dat maar één ding kan zijn.
    #[error(
        "cel '{cell}': besluit '{besluit}' leest veld '{field}' uit eerder besluit \
         '{earlier}', maar een gram van dat besluit draagt die naam twee keer — als \
         uitkomst of vast veld én als input"
    )]
    AmbiguousEarlierBesluitField {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit dat terugleest.
        besluit: String,
        /// Het besluit waaruit gelezen wordt.
        earlier: String,
        /// Het veld dat gelezen zou worden.
        field: String,
    },

    /// Een vraag over de celgrens geeft `$zaakkenmerk` mee, maar het besluit
    /// heeft geen zaakkenmerk-sjabloon.
    ///
    /// Dan is er niets in te vullen, en zou de bevraagde cel een lege tekst als
    /// zaak krijgen. Bij het optuigen, want het hangt niet van de zaak af.
    #[error(
        "cel '{cell}': besluit '{besluit}' geeft '$zaakkenmerk' mee in de vraag voor \
         input '{input}', maar de definitie heeft geen zaakkenmerk-sjabloon"
    )]
    ZaakkenmerkReferenceWithoutTemplate {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit dat de vraag stelt.
        besluit: String,
        /// De input die met het antwoord gevuld zou worden.
        input: String,
    },

    /// `$zaakkenmerk` staat naast een gedocumenteerde parameter met dezelfde
    /// naam.
    ///
    /// Eén naam voor twee dingen: het ingevulde kenmerk van dit besluit, en wat
    /// de aanroeper meegaf. De ingebouwde verwijzing wint, en dan zou de
    /// parameter stil iets anders betekenen dan er staat.
    #[error(
        "cel '{cell}': besluit '{besluit}' gebruikt '$zaakkenmerk' in de vraag voor input \
         '{input}' én documenteert een parameter 'zaakkenmerk'; die naam kan niet twee \
         dingen betekenen"
    )]
    AmbiguousZaakkenmerkReference {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit dat de vraag stelt.
        besluit: String,
        /// De input die met het antwoord gevuld zou worden.
        input: String,
    },

    /// Een besluit kan een van zijn inputs op dit moment niet ophalen.
    ///
    /// Geen "niets vastgesteld" zoals bij een reductie, maar een fout: een
    /// besluit dat een feit nodig heeft en het niet heeft, mag niet met een
    /// gat verder rekenen en al niet vastleggen.
    #[error("cel '{cell}': besluit '{besluit}' mist input '{input}': {reason}")]
    BesluitInputMissing {
        /// De besluitende cel.
        cell: String,
        /// Het besluit dat de input nodig had.
        besluit: String,
        /// De input die niet op te halen was.
        input: String,
        /// Waarom niet, in de woorden van de cel.
        reason: String,
    },

    /// Een besluit legt een uitkomst vast onder de naam van een vast veld van
    /// een decretogram.
    ///
    /// Dan zou de uitkomst dat veld overschrijven: het gram zou bijvoorbeeld
    /// zeggen dat de regeling `true` heet, of zijn receipt kwijt zijn. Een
    /// naamsbotsing hoort bij het optuigen te blijken en niet stil in een gram te
    /// eindigen.
    #[error(
        "cel '{cell}': besluit '{besluit}' legt uitkomst '{output}' vast, maar zo heet \
         een vast veld van elk decretogram ({fixed})"
    )]
    ReservedDecretogramField {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de botsende uitkomst.
        besluit: String,
        /// De uitkomstnaam die botst.
        output: String,
        /// Komma-gescheiden lijst van de vaste velden.
        fixed: String,
    },

    /// Een verplichting noemt een bedrag dat geen uitkomst van haar besluit is.
    ///
    /// Wat betaald moet worden, komt uit de wet die het besluit uitvoert. Een
    /// letterlijk bedrag of een parameter zou naast die uitkomst gaan leven, en
    /// dan zegt het gram twee dingen over hetzelfde geld.
    #[error(
        "cel '{cell}': verplichting uit {origin} noemt bedrag '{amount}'; dat moet \
         als $naam verwijzen naar een uitkomst van dat artikel of naar een \
         uitkomst die besluit '{besluit}' erbij vastlegt (uitkomsten: {outputs})"
    )]
    ObligationAmount {
        /// Cel waarin het besluit staat dat dit artikel uitvoert.
        cell: String,
        /// Het besluit dat het artikel uitvoert.
        besluit: String,
        /// Het lexogram dat de verplichting declareert.
        origin: String,
        /// Wat er als bedrag stond.
        amount: String,
        /// Komma-gescheiden lijst van de uitkomsten die er wél zijn.
        outputs: String,
    },

    /// Een verplichting noemt een soort die de opstelling niet kent.
    ///
    /// Platformvocabulaire, net als de ritmes: een soort erbij is een variant
    /// erbij, en een typfout hoort niet stil als betaling te eindigen. Een
    /// terugvordering staat er met opzet niet bij: die is niet te declareren maar
    /// volgt uit de richting van de verplichting.
    #[error(
        "cel '{cell}': verplichting uit {origin}, uitgevoerd door besluit \
         '{besluit}', is van soort '{soort}' (te declareren: {known}; een \
         terugvordering ontstaat uit `richting_bij_negatief: omkeren`)"
    )]
    UnknownObligationKind {
        /// Cel waarin het besluit staat dat dit artikel uitvoert.
        cell: String,
        /// Het besluit dat het artikel uitvoert.
        besluit: String,
        /// Het lexogram dat de verplichting declareert.
        origin: String,
        /// De onbekende soort.
        soort: String,
        /// Komma-gescheiden lijst van de soorten die er wél zijn.
        known: String,
    },

    /// Het `chronolex`-blok van een artikel is niet te lezen.
    ///
    /// Een blok dat er staat maar niet klopt, is een fout in de **wet** en niet
    /// in een wereldbestand. Stil overslaan zou een regeling laten zwijgen waar
    /// ze spreekt, en dan zou een besluit zonder verplichting niets bijzonders
    /// lijken.
    ///
    /// De bekende sleutels staan in de melding: de namespace is gesloten, dus
    /// wie hem geweigerd ziet worden hoort te lezen wat er dan wél in mag. Welke
    /// andere namespaces een `extensions` draagt, doet er niet toe — die blijven
    /// ongelezen (RFC-022 §3.2).
    #[error(
        "{origin}: het `extensions.chronolex`-blok is niet te lezen: {reason}; de \
         sleutels die dit blok kent zijn: {known}"
    )]
    MalformedChronolexBlock {
        /// Het lexogram met het blok: regeling, artikel en versie.
        origin: String,
        /// Wat er mis is.
        reason: String,
        /// Komma-gescheiden lijst van de sleutels die de namespace kent.
        known: String,
    },

    /// Een besluit-definitie draagt nog zelf `obligations`.
    ///
    /// Wat een besluit oplegt is normatief: het staat in de regeling die het
    /// uitvoert en niet in het wereldbestand van één uitvoerder. Zou het hier
    /// mogen blijven staan, dan konden twee uitvoerders van dezelfde regeling
    /// een ander betalingsschema hanteren zonder dat de wet verschilt.
    #[error(
        "cel '{cell}': besluit-definitie '{besluit}' draagt `obligations`; \
         verplichtingen staan in het lexogram, in het artikel van regeling \
         '{regulation}' dat uitkomst '{output}' voortbrengt, onder \
         `produces.extensions.chronolex.verplichtingen` — het wereldbestand \
         zegt alleen nog welke cel ze nakomt (`komt_na`)"
    )]
    ObligationsInWorldFile {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met het vervallen veld.
        besluit: String,
        /// De regeling die het besluit uitvoert.
        regulation: String,
        /// De uitkomst die het besluit aanstuurt.
        output: String,
    },

    /// Een verplichting wijst naar een partij die niet te lezen of niet in te
    /// vullen is.
    ///
    /// Eén melding voor beide kanten van de rechtsverhouding, met de rol erbij:
    /// wie een schuldenaar opschrijft die geen verwijzing is, en wie een
    /// schuldeiser weglaat waar er niets te leiden valt, heeft hetzelfde probleem
    /// — er staat geen partij.
    #[error(
        "cel '{cell}': de {role} van een verplichting van besluit '{besluit}' \
         ({origin}): {reason}"
    )]
    ObligationParty {
        /// Cel die besluit.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// Het lexogram dat de verplichting declareert.
        origin: String,
        /// Welke van de twee rollen het is.
        role: String,
        /// Wat er staat en waarom dat geen partij oplevert.
        reason: String,
    },

    /// Een verplichting rekent een negatief bedrag uit zonder dat de wet zegt wat
    /// dat betekent.
    ///
    /// Een negatieve betaling bestaat niet. Wat een vaststelling lager dan het
    /// voorschot oplevert, is juridisch een **terugvordering** (Awb 4:57): een
    /// verplichting de andere kant op, met de partij als schuldenaar. Dat is een
    /// andere rechtsverhouding en geen minteken, dus de wet moet hem declareren.
    /// Zwijgt ze, dan valt het besluit hier om en wordt er niets vastgelegd — een
    /// gram met een negatieve termijn erin zou een betaling beloven die niemand
    /// kan doen.
    #[error(
        "cel '{cell}': besluit '{besluit}' rekent op '{output}' een bedrag van \
         {bedrag} uit, en een verplichting kan niet negatief zijn; declareer \
         `richting_bij_negatief: omkeren` bij de verplichting in {origin} als een \
         negatief bedrag een terugvordering hoort te worden"
    )]
    NegativeObligationAmount {
        /// Cel die besloot.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// Het lexogram dat de verplichting declareert.
        origin: String,
        /// De uitkomst waaruit het bedrag kwam, zoals de declaratie haar noemt.
        output: String,
        /// Het bedrag dat eruit kwam.
        bedrag: String,
    },

    /// Een verplichting onder een regeling die geen bevoegd gezag aanwijst.
    ///
    /// De schuldenaar is standaard het bevoegd gezag, dus een regeling die
    /// daarover zwijgt laat de verplichting zonder partij. Anders dan bij een
    /// besluit zónder verplichting is dat geen gat om over te waarschuwen: er
    /// staat een verplichting, en er is niemand om haar aan te hangen.
    #[error(
        "cel '{cell}': besluit '{besluit}' legt een verplichting op uit {origin}, \
         maar die regeling wijst geen bevoegd gezag aan; dan is er geen naam voor \
         de schuldenaar"
    )]
    ObligationWithoutAuthority {
        /// Cel die besluit.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// Het lexogram dat de verplichting declareert.
        origin: String,
    },

    /// Twee cellen komen hetzelfde bevoegd gezag na.
    ///
    /// Dan is niet te zeggen welke van de twee betaalt, en een keuze die het
    /// platform maakt zou een betaling bij een willekeurige organisatie laten
    /// landen.
    #[error(
        "cellen '{cell}' en '{other}' komen allebei '{authority}' na; per gezag \
         kan één cel de betalingsverplichtingen nakomen"
    )]
    DuplicatePayerBinding {
        /// De cel die het gezag als tweede noemt.
        cell: String,
        /// De cel die het al noemde.
        other: String,
        /// Het gezag dat ze allebei noemen.
        authority: String,
    },

    /// De uitkomst waarnaar een verplichting verwijst is geen bedrag.
    ///
    /// Bij het besluit, niet bij het optuigen: dat de uitkomst bestaat is daar al
    /// getoetst, maar welke waarde ze heeft blijkt pas als de engine gedraaid
    /// heeft. Een schema uit een ontbrekende of niet-numerieke uitkomst zou een
    /// betaling van "niets" opleveren.
    #[error(
        "cel '{cell}': besluit '{besluit}' kan geen betalingsschema maken, want \
         uitkomst '{output}' is {found}"
    )]
    ObligationAmountValue {
        /// Cel die besloot.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// De uitkomst die het bedrag moest leveren.
        output: String,
        /// Wat er in plaats van een bedrag stond.
        found: String,
    },

    /// Een verplichting noemt een ritme dat de simulator niet kent.
    #[error(
        "cel '{cell}': verplichting van besluit '{besluit}' noemt ritme '{schedule}' \
         (bekend: {known})"
    )]
    UnknownSchedule {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// Het onbekende ritme.
        schedule: String,
        /// Komma-gescheiden lijst van de ritmes die wél bestaan.
        known: String,
    },

    /// Een verplichting verwijst met `ritme: $naam` naar iets dat er niet is: geen
    /// uitkomst van het besluit, en geen instelling van het wereldbestand.
    ///
    /// Een uitkomst gaat voor, dus de melding noemt die eerst: wie hier een
    /// typfout maakte, kan beide lijsten naast de naam leggen.
    #[error(
        "cel '{cell}': verplichting van besluit '{besluit}' verwijst naar ritme \
         '${setting}', maar dat is geen uitkomst van het uitvoerende artikel of van \
         `outputs` van het besluit (wel: {outputs}) en ook geen instelling in \
         `settings` van het wereldbestand (wel: {known})"
    )]
    UnknownSetting {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// De naam waarnaar verwezen wordt.
        setting: String,
        /// Komma-gescheiden lijst van de uitkomsten die het besluit kent.
        outputs: String,
        /// Komma-gescheiden lijst van de instellingen die er wél zijn.
        known: String,
    },

    /// `ritme: $naam` is zowel een uitkomst van het besluit als een instelling
    /// van het wereldbestand.
    ///
    /// De uitkomst zou voorgaan, en dan staat er een instelling in het
    /// wereldbestand die stil niets doet: wie haar wijzigt, verwacht een ander
    /// ritme en krijgt het niet. Daarom geen voorrang maar een weigering, bij het
    /// optuigen.
    #[error(
        "cel '{cell}': verplichting van besluit '{besluit}' ({origin}) verwijst naar \
         ritme '${name}', en die naam is zowel een uitkomst van {output_origin} als \
         een instelling in `settings` van het wereldbestand; hernoem een van beide, \
         want een uitkomst gaat voor en de instelling zou stil niets doen"
    )]
    AmbiguousScheduleReference {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// Regeling, versie en artikel van de verplichting.
        origin: String,
        /// De naam die op beide plekken bestaat.
        name: String,
        /// Waar de uitkomst staat: het uitvoerende artikel, of `outputs` van het
        /// besluit.
        output_origin: String,
    },

    /// Het ritme van een verplichting komt uit een uitkomst, en die uitkomst is
    /// geen ritme.
    ///
    /// Bij het besluit, niet bij het optuigen: welke waarde de uitkomst heeft,
    /// blijkt pas als de engine gedraaid heeft. Het besluit valt om en er wordt
    /// niets vastgelegd — een schema in een ritme dat niet bestaat, zou stil
    /// "ineens" of niets worden.
    #[error(
        "cel '{cell}': besluit '{besluit}' kan geen betalingsschema maken, want het \
         ritme komt uit uitkomst '{output}' en die is {found} (toegestaan: {known})"
    )]
    ScheduleOutputValue {
        /// Cel die besloot.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// De uitkomst die het ritme moest leveren.
        output: String,
        /// Wat er in plaats van een ritme stond.
        found: String,
        /// Komma-gescheiden lijst van de ritmes die wél bestaan.
        known: String,
    },

    /// De `from` van een verplichting levert geen datum op.
    #[error(
        "cel '{cell}': verplichting van besluit '{besluit}' begint bij '{template}', \
         maar {reason}"
    )]
    MalformedObligationDate {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// Het sjabloon zoals het in de configuratie staat.
        template: String,
        /// Wat er mis is.
        reason: String,
    },

    /// Een cel die een verplichting moet dragen houdt geen betalingsstroom.
    ///
    /// Beide kanten van een verplichting leggen vast: de betalende cel dat ze
    /// betaalde, de besluitende dat het haar gemeld is. Dat kan alleen in een
    /// stroom die er is, met de sleutel waarop een zaak terug te vinden is — en
    /// dat hoort bij het optuigen te blijken en niet op de eerste vervaldatum.
    #[error(
        "cel '{cell}': verplichting van besluit '{besluit}' vraagt dat cel '{holder}' \
         {expected} houdt, maar {found}"
    )]
    ObligationStream {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// De cel die de stroom zou moeten houden.
        holder: String,
        /// De stroom met de sleutel die het platform verwacht.
        expected: String,
        /// Wat er in plaats daarvan is.
        found: String,
    },

    /// Het zaakkenmerk-sjabloon heeft een accolade die niet sluit.
    #[error(
        "cel '{cell}': zaakkenmerk '{template}' van besluit '{besluit}' heeft een \
         accolade die niet sluit; een verwijzing schrijf je als {{parameter}}"
    )]
    MalformedZaakkenmerk {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met het kapotte sjabloon.
        besluit: String,
        /// Het sjabloon zoals het in de configuratie staat.
        template: String,
    },

    /// Het zaakkenmerk-sjabloon verwijst naar geen enkele parameter.
    ///
    /// Dan krijgt elke zaak hetzelfde kenmerk, en valt het ene besluit niet van
    /// het andere te onderscheiden: een reductie over de stroom zou het laatste
    /// besluit over wie dan ook opleveren.
    #[error(
        "cel '{cell}': zaakkenmerk '{template}' van besluit '{besluit}' verwijst naar \
         geen enkele parameter, dus elke zaak zou hetzelfde kenmerk krijgen"
    )]
    ZaakkenmerkWithoutReference {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met het te algemene sjabloon.
        besluit: String,
        /// Het sjabloon zoals het in de configuratie staat.
        template: String,
    },

    /// Twee verwijzingen in een zaakkenmerk-sjabloon plakken aan elkaar.
    ///
    /// `{jaar}{bsn}` levert voor 2024 + 999993653 hetzelfde kenmerk als voor
    /// 20249 + 99993653: twee zaken, één kenmerk, en een reductie die het besluit
    /// van de ander teruggeeft. Geen enkele parameterwaarde kan dat repareren,
    /// dus het is een optuigfout.
    #[error(
        "cel '{cell}': zaakkenmerk '{template}' van besluit '{besluit}' zet '{first}' \
         en '{second}' tegen elkaar aan; zet er iets tussen dat ze scheidt"
    )]
    AdjacentZaakkenmerkReferences {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met het dubbelzinnige sjabloon.
        besluit: String,
        /// Het sjabloon zoals het in de configuratie staat.
        template: String,
        /// De eerste van de twee verwijzingen.
        first: String,
        /// De verwijzing die er direct achter staat.
        second: String,
    },

    /// Een parameterwaarde bevat het scheidingsteken van het zaakkenmerk-sjabloon.
    ///
    /// Dan valt het kenmerk niet meer eenduidig terug te lezen: bij
    /// `{jaar}/{bsn}` geeft `2024/9` + `99993653` hetzelfde kenmerk als `2024` +
    /// `999993653`. Het zaakkenmerk is waaronder een zaak terug te vinden is, dus
    /// twee zaken mogen er nooit één worden.
    #[error(
        "cel '{cell}': besluit '{besluit}' krijgt voor parameter '{parameter}' een \
         waarde met '{separator}' erin, en dat scheidt in zaakkenmerk '{template}' \
         twee verwijzingen; dan zou dit kenmerk ook bij een andere zaak kunnen horen"
    )]
    ZaakkenmerkSeparatorInValue {
        /// De besluitende cel.
        cell: String,
        /// Het besluit dat genomen werd.
        besluit: String,
        /// Het sjabloon zoals het in de configuratie staat.
        template: String,
        /// De parameter met de dubbelzinnige waarde.
        parameter: String,
        /// Het scheidingsteken dat in die waarde voorkomt.
        separator: String,
    },

    /// De stand van een kroniekstroom kon niet gehasht worden.
    ///
    /// Een fout en geen stille constante: een hash die er als een hash uitziet
    /// maar niets identificeert, zou in een decretogram voor bewijs doorgaan.
    #[error("kon de stand van kroniekstroom '{stream}' niet hashen: {source}")]
    ChronicleHashing {
        /// De stroom waarvan de grammen niet te serialiseren waren.
        stream: String,
        /// De onderliggende serialisatiefout.
        source: serde_yaml_ng::Error,
    },

    /// De regeling wijst een bevoegd gezag aan met een `#`-verwijzing die
    /// nergens op uitkomt.
    ///
    /// Dat is iets anders dan een regeling die zwijgt: er ís een gezag
    /// gedeclareerd, alleen niet te lezen. Doorgaan alsof de wet niets zegt zou
    /// de toets op het bevoegd gezag stil uitzetten, en dat is precies de kant
    /// die niet mag: dan besluit iedereen.
    #[error(
        "cel '{cell}': regeling '{regulation}' wijst het bevoegd gezag aan als '#{reference}', \
         maar geen actie van die regeling zet die uitkomst op een letterlijke naam \
         (besluit '{besluit}')"
    )]
    CompetentAuthorityUnresolvable {
        /// De cel die wilde besluiten.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// De regeling, bij `$id`.
        regulation: String,
        /// De uitkomstnaam achter de `#`.
        reference: String,
    },

    /// Het receipt van een besluit kon niet als kroniekveld worden opgeslagen.
    #[error("cel '{cell}': kon het receipt van besluit '{besluit}' niet vastleggen: {source}")]
    ReceiptEncoding {
        /// De besluitende cel.
        cell: String,
        /// Het besluit waarvan het receipt niet opgeslagen kon worden.
        besluit: String,
        /// De onderliggende serialisatiefout.
        source: serde_yaml_ng::Error,
    },

    /// Geen regelingmap met deze naam in het corpus.
    #[error("regeling '{regulation}' niet gevonden onder {root}")]
    RegulationNotFound {
        /// De gezochte regeling.
        regulation: String,
        /// De corpus-wortel waarin gezocht is.
        root: PathBuf,
    },

    /// De map heet anders dan de `$id` in het document.
    #[error("{path}: document heeft $id '{found}', maar staat in map '{expected}'")]
    RegulationIdMismatch {
        /// Het gelezen bestand.
        path: PathBuf,
        /// De naam van de map (en dus van de wet in de celconfiguratie).
        expected: String,
        /// De `$id` die het document zelf opgeeft.
        found: String,
    },

    /// Het schema van een stroom noemt dezelfde gebeurtenis twee keer.
    ///
    /// Om dezelfde reden als bij [`Self::DuplicateStream`]: elke toets zoekt de
    /// eerste gebeurtenis die bij de naam past, dus de tweede declaratie doet
    /// stil niets. Wie twee vormen van hetzelfde feit bedoelt, geeft ze twee
    /// namen; wie hem per ongeluk twee keer opschreef, hoort dat te horen.
    #[error(
        "cel '{cell}', kroniekstroom '{stream}': het schema noemt gebeurtenis \
         '{name}' twee keer"
    )]
    DuplicateGebeurtenis {
        /// De cel die de stroom houdt.
        cell: String,
        /// De stroom met het schema.
        stream: String,
        /// De naam die dubbel staat.
        name: String,
    },

    /// Een vastlegging noemt een gebeurtenis die het schema van haar stroom niet
    /// kent.
    ///
    /// Een stroom die een schema declareert, zegt daarmee wélke gebeurtenissen
    /// erin thuishoren. Een naam die er niet in staat is een typfout of een feit
    /// dat in een andere stroom hoort; beide zouden anders stil in de kroniek
    /// belanden, en dan is er niets meer dat over die vastlegging iets belooft.
    #[error(
        "cel '{cell}', kroniekstroom '{stream}': het schema van die stroom kent geen \
         gebeurtenis '{name}' (wel: {known})"
    )]
    UnknownGebeurtenis {
        /// De cel die de stroom houdt.
        cell: String,
        /// De stroom met het schema.
        stream: String,
        /// De naam die de vastlegging noemt.
        name: String,
        /// Komma-gescheiden lijst van de gedeclareerde gebeurtenissen.
        known: String,
    },

    /// Een vastlegging mist een veld dat haar gebeurtenisschema declareert.
    #[error(
        "cel '{cell}', kroniekstroom '{stream}': gebeurtenis '{name}' mist veld \
         '{field}' ({expected}), dat het schema van die gebeurtenis declareert"
    )]
    GebeurtenisVeldOntbreekt {
        /// De cel die de stroom houdt.
        cell: String,
        /// De stroom met het schema.
        stream: String,
        /// De gebeurtenis waarvan het schema geldt.
        name: String,
        /// Het veld dat ontbreekt.
        field: String,
        /// Het gedeclareerde type van dat veld.
        expected: &'static str,
    },

    /// Een veld van een vastlegging is van een ander soort dan het schema
    /// declareert.
    ///
    /// Los van [`Self::GebeurtenisVeldDatum`], en om dezelfde reden als bij een
    /// parameter (zie [`Self::ParameterType`] en [`Self::ParameterDate`]): "dit
    /// is een getal en er hoorde tekst te staan" is iets anders dan "dit is
    /// tekst, maar geen datum". Eén melding voor beide zou bij een datum altijd
    /// "verwacht date, kreeg string" opleveren, en dat is waar voor wie
    /// `01-12-2026` schreef — en nutteloos.
    ///
    /// Zonder het cel-id, net als [`Self::ChronicleEventWithoutKey`]: de stroom,
    /// de gebeurtenis en het veld wijzen de plek aan, en wie een wereldbestand
    /// leest heeft aan die drie genoeg om de regel te vinden.
    #[error(
        "kroniekstroom '{stream}': veld '{field}' van gebeurtenis '{name}' is \
         {actual}, en het schema declareert {expected}"
    )]
    GebeurtenisVeldType {
        /// De stroom met het schema.
        stream: String,
        /// De gebeurtenis waarvan het schema geldt.
        name: String,
        /// Het veld met de waarde die niet past.
        field: String,
        /// Het gedeclareerde type.
        expected: &'static str,
        /// Het soort waarde dat er staat.
        actual: &'static str,
    },

    /// Een veld dat een datum hoort te zijn, staat niet in ISO-notatie.
    #[error(
        "kroniekstroom '{stream}': veld '{field}' van gebeurtenis '{name}' is \
         gedeclareerd als date, en '{value}' staat niet in de notatie jjjj-mm-dd"
    )]
    GebeurtenisVeldDatum {
        /// De stroom met het schema.
        stream: String,
        /// De gebeurtenis waarvan het schema geldt.
        name: String,
        /// Het veld met de waarde die niet past.
        field: String,
        /// De waarde zoals ze er staat.
        value: String,
    },

    /// Een kroniekgebeurtenis mist het sleutelveld van haar stroom.
    #[error("kroniekstroom '{stream}': gebeurtenis van {op_moment} mist sleutelveld '{key}'")]
    ChronicleEventWithoutKey {
        /// De stroom waarin de gebeurtenis staat.
        stream: String,
        /// Het sleutelveld dat de stroom declareert.
        key: String,
        /// Het moment van de gebeurtenis.
        op_moment: String,
    },

    /// Een vastlegging staat op naam van een andere cel dan die haar houdt.
    ///
    /// Een kroniek is het eigen journaal van de celbeheerder (RFC-022 §1.3):
    /// wat de cel zelf overkwam, door haar vastgelegd. Een vastlegging op naam
    /// van een ander is geen feit maar een aanname over een ander.
    #[error(
        "cel '{cell}': vastlegging van {op_moment} in stroom '{stream}' staat op naam van \
         '{recording_actor}', maar een kroniek houdt alleen de eigen vastleggingen van de cel"
    )]
    ForeignRecordingActor {
        /// De cel die de stroom houdt.
        cell: String,
        /// De stroom waarin de vastlegging staat.
        stream: String,
        /// De actor die de vastlegging opgeeft.
        recording_actor: String,
        /// Het moment van de vastlegging.
        op_moment: String,
    },

    /// Er wordt vastgelegd in een stroom die de cel niet houdt.
    #[error("cel '{cell}' houdt geen kroniekstroom '{stream}' (wel: {known})")]
    UnknownChronicleStream {
        /// De cel waarin vastgelegd werd.
        cell: String,
        /// De gevraagde stroomnaam.
        stream: String,
        /// Komma-gescheiden lijst van stromen die de cel wél houdt.
        known: String,
    },

    /// De klok van de wereld zou achteruit moeten lopen.
    ///
    /// Een logische klok gaat één kant op. Wie het beeld van een eerder moment
    /// wil, vraagt dat op met `op_moment`; daarvoor hoeft de wereld niet terug.
    #[error("de klok van de wereld staat op {clock} en loopt niet terug naar {to}")]
    ClockRunsBackwards {
        /// Waar de klok nu staat.
        clock: String,
        /// Het moment waarnaar gevraagd werd.
        to: String,
    },

    /// Er wordt gereduceerd of besloten over een moment dat nog niet gebeurd is.
    ///
    /// De wereld weet niet wat er na haar klok gebeurt: triggers die nog moeten
    /// afgaan hebben niets vastgelegd. Een antwoord "op" zo'n moment zou een
    /// voorspelling zijn die zich voordoet als een reductie, en een besluit "op"
    /// zo'n moment zou een gram in de toekomst leggen.
    #[error(
        "cel '{cell}': {subject} '{name}' op {op_moment} vraagt een moment ná de klok \
         van de wereld ({clock})"
    )]
    MomentAfterClock {
        /// De bevraagde cel.
        cell: String,
        /// Of dit over een lexostatus of over een besluit gaat.
        subject: Subject,
        /// De gevraagde lexostatus of het uitgevoerde besluit.
        name: String,
        /// Het gevraagde moment.
        op_moment: String,
        /// Waar de klok staat.
        clock: String,
    },

    /// Twee kroniekstromen met dezelfde naam in één cel.
    ///
    /// De stroomnaam is tevens de naam van de databron in de engine. Twee
    /// stromen met dezelfde naam schaduwen elkaar daar stil; dat is een
    /// configuratiefout en geen keuze.
    #[error("cel '{cell}' definieert kroniekstroom '{stream}' twee keer")]
    DuplicateStream {
        /// Cel waarin het dubbel staat.
        cell: String,
        /// De dubbele stroomnaam.
        stream: String,
    },

    /// Twee cellen met hetzelfde id in één scenario.
    #[error("scenario definieert cel '{cell}' twee keer")]
    DuplicateCell {
        /// Het dubbele cel-id.
        cell: String,
    },

    /// Een vraag in het scenario legt niets vast wat ze moet opleveren.
    ///
    /// De assertie hoort bij het scenario. Een vraag zonder `expect` slaagt
    /// altijd en bewijst niets; dat mag geen groen opleveren.
    #[error(
        "scenario '{scenario}': vraag naar '{cell}.{lexostatus}' heeft geen `expect` \
         en geen `expect_not_established`, en bewijst dus niets"
    )]
    QueryWithoutExpectation {
        /// Het scenario waarin de vraag staat.
        scenario: String,
        /// De bevraagde cel.
        cell: String,
        /// De gevraagde lexostatus.
        lexostatus: String,
    },

    /// Een vraag verwacht waarden én dat er niets vastgesteld is.
    ///
    /// Die twee sluiten elkaar uit, dus zo'n vraag kan nooit slagen. Dat is een
    /// schrijffout in het scenario en geen falende assertie: hij blijkt bij het
    /// lezen, niet pas na een run.
    #[error(
        "scenario '{scenario}': vraag naar '{cell}.{lexostatus}' verwacht zowel waarden \
         als `expect_not_established`; dat kan niet samen uitkomen"
    )]
    ContradictoryExpectation {
        /// Het scenario waarin de vraag staat.
        scenario: String,
        /// De bevraagde cel.
        cell: String,
        /// De gevraagde lexostatus.
        lexostatus: String,
    },

    /// Het gedeclareerde vraaggraf noemt een cel die het scenario niet heeft.
    ///
    /// Bij het lezen en niet pas na een run: zo'n tak wordt nooit gesteld en zou
    /// anders als "gedeclareerde vraag die uitbleef" naar buiten komen — een
    /// melding die de lezer naar de run stuurt terwijl er een typfout in het
    /// bestand staat.
    #[error(
        "scenario '{scenario}': het vraaggraf declareert '{edge}', maar cel '{cell}' bestaat \
         in dit scenario niet (wel: {known})"
    )]
    QueryGraphUnknownCell {
        /// Het scenario waarin de declaratie staat.
        scenario: String,
        /// De gedeclareerde tak.
        edge: String,
        /// De cel die niet bestaat.
        cell: String,
        /// Komma-gescheiden lijst van cellen die het scenario wél kent.
        known: String,
    },

    /// Een tak van het vraaggraf laat een cel zichzelf bevragen.
    ///
    /// Dezelfde weigering als [`SimulatorError::TransportToSelf`], een stap
    /// eerder: zo'n tak kan nooit uitkomen, want de veiligheidscontext laat een
    /// vraag aan de eigen cel niet over de grens.
    #[error(
        "scenario '{scenario}': het vraaggraf declareert '{edge}', maar een cel bevraagt \
         zichzelf niet over een celgrens; voor eigen feiten is er een reductie"
    )]
    QueryGraphToSelf {
        /// Het scenario waarin de declaratie staat.
        scenario: String,
        /// De gedeclareerde tak.
        edge: String,
    },

    /// Dezelfde tak staat twee keer in het vraaggraf.
    ///
    /// Een graf is een verzameling, dus de tweede regel voegt niets toe. Stil
    /// samenvoegen zou betekenen dat er een regel in het bestand staat die niets
    /// doet, en dat is precies wat een wereldbestand niet hoort te hebben.
    #[error("scenario '{scenario}': het vraaggraf declareert '{edge}' twee keer")]
    DuplicateQueryGraphEdge {
        /// Het scenario waarin de declaratie staat.
        scenario: String,
        /// De dubbele tak.
        edge: String,
    },

    /// Een vraag richt zich tot een cel die het scenario niet kent.
    #[error("scenario kent geen cel '{cell}'")]
    UnknownCell {
        /// Het onbekende cel-id.
        cell: String,
    },

    /// Het transport kent de aangewezen peer niet.
    ///
    /// Eigen variant naast [`SimulatorError::UnknownCell`]: dit is geen fout in
    /// een scenariobestand maar een vraag die de grens over wilde naar iets wat
    /// er niet is. Een transport verzint geen antwoord.
    #[error("transport kent geen cel '{cell}' (wel: {known})")]
    UnknownPeer {
        /// Het onbekende cel-id.
        cell: String,
        /// Komma-gescheiden lijst van cellen die het transport wél bereikt.
        known: String,
    },

    /// Een besluit-definitie accepteert een input van de eigen cel.
    ///
    /// Voor eigen feiten is er een kroniek of een eigen wet. Zou dit mogen, dan
    /// zou een cel zichzelf over de grens bevragen en als cross-cel-contact in
    /// het vraaggraf komen — zie [`SimulatorError::TransportToSelf`], dezelfde
    /// weigering een stap eerder.
    #[error(
        "cel '{cell}': besluit '{besluit}' accepteert input '{input}' van de eigen cel; \
         voor eigen feiten is er een kroniek of een eigen wet"
    )]
    AcceptFromSelf {
        /// De cel waarin de definitie staat.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// De input die zichzelf zou bevragen.
        input: String,
    },

    /// Een cel declareert een cel-bron die geen van haar wetten aanwijst.
    ///
    /// Een cel bevraagt alleen de cellen die haar eigen wetten via
    /// `source.regulation` noemen (invariant I3). Een afspraak daarbuiten zou
    /// een vraaggraf openzetten waar het recht niet om vraagt, en dat hoort niet
    /// pas tijdens een besluit te blijken.
    #[error(
        "cel '{cell}': `accepts_from` noemt '{peer}.{output}', maar geen van de eigen \
         wetten vraagt daarom (wel: {asked})"
    )]
    UnclaimedCellSource {
        /// De cel waarin de afspraak staat.
        cell: String,
        /// De gedeclareerde bron-cel.
        peer: String,
        /// De gedeclareerde uitkomst.
        output: String,
        /// Komma-gescheiden lijst van cel-bronnen die de wetten wél noemen.
        asked: String,
    },

    /// Twee afspraken over dezelfde cel-bron en uitkomst.
    #[error("cel '{cell}': `accepts_from` noemt '{peer}.{output}' twee keer")]
    DuplicateCellSource {
        /// De cel waarin de afspraak staat.
        cell: String,
        /// De bron-cel die dubbel staat.
        peer: String,
        /// De uitkomst die dubbel staat.
        output: String,
    },

    /// Een gedeclareerde cel-bron heet net zo als een eigen regeling.
    ///
    /// Dan zou een vraag die voor de andere organisatie bedoeld is door de
    /// gelijknamige regeling beantwoord worden, zonder spoor van de omleiding.
    /// De engine weigert dat ook (RFC-022 §4.2); hier valt het bij het optuigen,
    /// zodat de cel niet eerst hoeft te besluiten om het te merken.
    #[error(
        "cel '{cell}': `accepts_from` noemt cel '{peer}', maar die naam is ook een \
         regeling die deze cel zelf laadt"
    )]
    CellShadowsRegulation {
        /// De cel waarin de afspraak staat.
        cell: String,
        /// Het cel-id dat een eigen regeling overschaduwt.
        peer: String,
    },

    /// Een cel wil accepteren van een cel die in deze wereld niet bestaat.
    ///
    /// Een cel kent geen andere cel, dus bij het optuigen van de cel valt dit
    /// niet op; de wereld kent ze wel allemaal en toetst het daarom hier. Zonder
    /// deze weigering zou een typfout in een peer-naam pas tijdens het besluit
    /// opduiken, en dan als "transport kent geen cel" — een melding die naar het
    /// transport wijst terwijl het bestand fout is.
    #[error(
        "cel '{cell}' wil {what} accepteren van cel '{peer}', maar die kent dit scenario \
         niet (wel: {known})"
    )]
    UnknownAcceptedCell {
        /// De cel die wil accepteren.
        cell: String,
        /// De peer die niet bestaat.
        peer: String,
        /// Wat er geaccepteerd zou worden, in woorden.
        what: String,
        /// Komma-gescheiden lijst van cellen die de wereld wél kent.
        known: String,
    },

    /// Een reductie heeft een waarde van een andere cel nodig.
    ///
    /// Dit is de andere kant van de twee engine-configuraties (RFC-022 §4.2): de
    /// reduce-engine heeft geen `CellResolver`, dus een `source.regulation` die
    /// een cel aanwijst is voor haar een onbekende regeling. De cel reikt niet
    /// buiten zichzelf om te kunnen antwoorden, en dat is geen gebrek maar het
    /// hele punt — accepteren van een ander is iets wat je bij een **besluit**
    /// doet, met herkomst in het decretogram, niet stilletjes tijdens een vraag.
    #[error(
        "cel '{cell}': lexostatus '{lexostatus}' heeft een waarde van cel '{peer}' nodig, \
         maar een reductie reikt niet buiten de eigen cel; van een ander accepteren doet \
         een cel in een besluit"
    )]
    ReductionReachesOutsideCell {
        /// De cel waaraan gevraagd werd.
        cell: String,
        /// De lexostatus die niet te reduceren was.
        lexostatus: String,
        /// De cel waarvan de wet een waarde nodig had.
        peer: String,
    },

    /// Een wet van deze cel wijst een producent aan die de cel niet laadt en ook
    /// niet als cel-bron declareert.
    ///
    /// De engine zegt hier "regeling niet gevonden", en dat is letterlijk waar en
    /// tegelijk het verkeerde spoor: de naam staat in de wet, dus wie dit leest
    /// zoekt een ontbrekend corpusbestand terwijl er een afspraak mist. Welke van
    /// de twee het is, weet de cel niet — een `source.regulation` zegt niet of ze
    /// een regeling of een organisatie aanwijst (RFC-022 §4.2) — dus de melding
    /// noemt beide uitwegen en kiest er niet één.
    ///
    /// Dit valt niet bij het optuigen: een besluit-definitie mag zo'n input zelf
    /// aanleveren, en dan komt de verwijzing nooit aan bod.
    #[error(
        "cel '{cell}': besluit '{besluit}' voert een regeling uit die '{name}' aanwijst, \
         maar deze cel laadt geen regeling met die naam en declareert '{name}' ook niet \
         in `accepts_from`; laad de regeling, of leg in `accepts_from` vast welke \
         lexostatus bij die cel gevraagd moet worden"
    )]
    UndeclaredCellSource {
        /// De cel die wilde besluiten.
        cell: String,
        /// Het besluit dat de waarde nodig had.
        besluit: String,
        /// De naam uit `source.regulation` die nergens op uitkomt.
        name: String,
    },

    /// Een besluit kon een waarde niet van een andere cel accepteren.
    ///
    /// Eigen variant naast [`SimulatorError::BesluitInputMissing`], want de reden
    /// ligt buiten deze cel: de bron-cel stelde niets vast, of ze publiceert de
    /// gevraagde uitkomst niet. Het eerste is haar goed recht en geen defect —
    /// wat er niet mag gebeuren, is doorrekenen met een gat. Het besluit valt dus
    /// om en er wordt niets vastgelegd.
    ///
    /// De melding zegt wat de acceptatiecriteria vragen: welke input, van welke
    /// cel, en waarom het niet lukte.
    #[error(
        "cel '{cell}': besluit '{besluit}' kon input '{input}' niet accepteren van cel \
         '{peer}': {reason}"
    )]
    AcceptedInputMissing {
        /// De cel die wilde besluiten.
        cell: String,
        /// Het besluit dat de input nodig had.
        besluit: String,
        /// De input die ontbrak.
        input: String,
        /// De bevraagde cel.
        peer: String,
        /// Waarom de waarde niet aankwam, met de lexostatus en het moment erin.
        reason: String,
    },

    /// Een cel bevraagt zichzelf via het transport.
    ///
    /// Voor de eigen feiten is er een reductie; het transport is er voor peers.
    /// Zonder deze weigering zou een cel haar eigen kroniek als cross-cel-contact
    /// in het observatielog krijgen en daarmee het vraaggraf vervuilen.
    #[error(
        "cel '{cell}' vraagt '{lexostatus}' via het transport aan zichzelf; \
         voor eigen feiten is er een reductie"
    )]
    TransportToSelf {
        /// De cel die zichzelf bevroeg.
        cell: String,
        /// De gevraagde lexostatus.
        lexostatus: String,
    },

    /// Er is een actie aangeroepen die het wereldbestand niet kent.
    #[error("de wereld kent geen actie '{action}' (wel: {known})")]
    UnknownAction {
        /// De aangeroepen actie.
        action: String,
        /// Komma-gescheiden lijst van de acties die er wél zijn.
        known: String,
    },

    /// Het wereldbestand declareert twee acties met hetzelfde id.
    ///
    /// De tweede zou de eerste stil schaduwen: een actie wordt op haar id
    /// aangeroepen, dus dan zou er één zijn die niemand meer kan doen.
    #[error("de wereld declareert actie '{action}' twee keer")]
    DuplicateAction {
        /// Het dubbele actie-id.
        action: String,
    },

    /// Het portaal is voor een cel die deze wereld niet kent.
    ///
    /// Bij het optuigen, want anders staat er een pagina zonder één actie en
    /// zegt niemand waarom.
    #[error("portaal '{label}': actor '{actor}' is geen cel in deze wereld (wel: {known})")]
    PortaalUnknownActor {
        /// Het label van het portaal.
        label: String,
        /// De actor die niet bestaat.
        actor: String,
        /// Komma-gescheiden lijst van de cellen die er wél zijn.
        known: String,
    },

    /// Twee persona's in het portaal met hetzelfde id.
    ///
    /// Een persona wordt op haar id gekozen; de tweede zou onbereikbaar zijn.
    #[error("het portaal declareert persona '{persona}' twee keer")]
    DuplicatePersona {
        /// Het dubbele id.
        persona: String,
    },

    /// Een persona noemt een veld dat in geen enkel formulier van de actor
    /// voorkomt.
    ///
    /// Zo'n waarde vult nooit iets in, en dat zou niemand merken: het formulier
    /// houdt dan gewoon zijn eigen voorinvulling.
    #[error(
        "persona '{persona}' noemt veld '{field}', maar geen enkele actie van actor \
         '{actor}' heeft een veld met die naam (wel: {known})"
    )]
    PersonaFieldNotInForm {
        /// De persona.
        persona: String,
        /// Het veld dat nergens voorkomt.
        field: String,
        /// De actor van het portaal.
        actor: String,
        /// Komma-gescheiden lijst van de velden die er wél zijn.
        known: String,
    },

    /// De persona die gekozen wordt, staat niet in het portaal.
    #[error("het portaal kent geen persona '{persona}' (wel: {known})")]
    UnknownPersona {
        /// De gevraagde persona.
        persona: String,
        /// Komma-gescheiden lijst van de persona's die er wél zijn.
        known: String,
    },

    /// Er wordt een persona gekozen in een wereld zonder portaal.
    #[error("deze wereld heeft geen portaal, en dus geen persona om te kiezen")]
    NoPortaal,

    /// Een regel van het inzicht vraagt een cel die deze wereld niet kent.
    #[error("inzicht '{label}': de wereld kent geen cel '{cell}' (wel: {known})")]
    InzichtUnknownCell {
        /// Het label van de regel.
        label: String,
        /// De cel die niet bestaat.
        cell: String,
        /// Komma-gescheiden lijst van de cellen die er wél zijn.
        known: String,
    },

    /// Een regel van het inzicht vult andere parameters dan de lexostatus
    /// documenteert.
    ///
    /// Precies dezelfde namen, niet meer en niet minder: de cel weigert elke
    /// andere vraag, en dan zou de kaart op het scherm altijd een fout tonen.
    #[error(
        "inzicht '{label}': lexostatus '{cell}.{lexostatus}' vraagt [{expected}], maar de \
         regel vult [{given}]"
    )]
    InzichtParams {
        /// Het label van de regel.
        label: String,
        /// De bevraagde cel.
        cell: String,
        /// De bevraagde lexostatus.
        lexostatus: String,
        /// Komma-gescheiden lijst van wat de regel vult.
        given: String,
        /// Komma-gescheiden lijst van wat de lexostatus vraagt.
        expected: String,
    },

    /// Een sjabloon in een regel van het inzicht heeft een accolade die niet
    /// sluit.
    #[error(
        "inzicht '{label}': parameter '{parameter}' heeft sjabloon '{template}' met een \
         accolade die niet sluit; een verwijzing schrijf je als {{veld}}"
    )]
    InzichtMalformedTemplate {
        /// Het label van de regel.
        label: String,
        /// De parameter met het kapotte sjabloon.
        parameter: String,
        /// Het sjabloon zoals het in het wereldbestand staat.
        template: String,
    },

    /// Een sjabloon in een regel van het inzicht verwijst naar een veld dat een
    /// persona niet noemt.
    ///
    /// Per persona, want het inzicht wordt per persona ingevuld: een verwijzing
    /// die bij de ene wel en bij de andere niet uitkomt, levert bij die andere
    /// een vraag over een zaak die niet bestaat.
    #[error(
        "inzicht '{label}': parameter '{parameter}' verwijst naar {{{reference}}}, maar \
         persona '{persona}' noemt geen veld '{reference}'"
    )]
    InzichtUnknownReference {
        /// Het label van de regel.
        label: String,
        /// De parameter met het sjabloon.
        parameter: String,
        /// Het veld waarnaar verwezen wordt.
        reference: String,
        /// De persona die het veld niet noemt.
        persona: String,
    },

    /// Een actie kan op dit moment niet: het verhaal is nog niet zover.
    ///
    /// Geen vergissing van de aanroeper maar een stand van de wereld, en daarom
    /// een leesbare weigering: wát er nog niet vastligt staat erin.
    #[error("actie '{action}' kan nu niet: {reason}")]
    ActionNotAvailable {
        /// De actie die niet kan.
        action: String,
        /// Waarom niet, in woorden.
        reason: String,
    },

    /// De bekendmaking van een besluit kan niet: er ligt geen besluit dat erop
    /// wacht.
    ///
    /// Afgeleid en niet gedeclareerd: of er bekendgemaakt kan worden, staat in de
    /// eigen kronieken van de cel — er ligt een gram van de stage BESLUIT en
    /// (nog) geen van de stage BEKENDMAKING. De reden noemt daarom het gram dat
    /// ontbreekt of het gram dat er al ligt, en niet een voorwaarde die iemand
    /// had moeten opschrijven.
    #[error("cel '{cell}' kan besluit '{besluit}' niet bekendmaken: {reason}")]
    BekendmakingNietMogelijk {
        /// De cel die zou bekendmaken.
        cell: String,
        /// De besluit-definitie waarvan de bekendmaking gevraagd werd.
        besluit: String,
        /// Wat er aan de hand is: welk gram ontbreekt, of welk gram er al ligt.
        reason: String,
    },

    /// De regeling van dit besluit kent de stage BEKENDMAKING niet.
    ///
    /// Bekendmaken is een stap in een **procedure** (RFC-008), en welke stappen
    /// een beschikking kent, zegt de algemene wet en niet deze opstelling. Kent
    /// geen enkele geladen regeling zo'n procedure, dan valt er niets uit te
    /// voeren — en dan is een gram vastleggen alsof er wél een stage gedraaid is,
    /// precies de stilte die dit pad moet voorkomen.
    #[error(
        "cel '{cell}' kan besluit '{besluit}' niet bekendmaken: geen enkele geladen regeling \
         declareert voor rechtskarakter '{legal_character}' een procedure met een stage \
         '{stage}' ({reason})"
    )]
    GeenBekendmakingStage {
        /// De cel die zou bekendmaken.
        cell: String,
        /// De besluit-definitie waarvan de bekendmaking gevraagd werd.
        besluit: String,
        /// Het rechtskarakter waarvoor een procedure gezocht is.
        legal_character: String,
        /// De stage die ontbreekt.
        stage: String,
        /// Wat er wél gevonden is.
        reason: String,
    },

    /// De stage BEKENDMAKING kon niet draaien: er ontbreekt invoer.
    ///
    /// De procedure zegt wat een stage nodig heeft (`requires`), en de engine
    /// levert geen halve uitkomst maar de vraag om precies die invoer. Wat het
    /// platform zelf aanreikt — de dag van de bekendmaking, het bevoegd gezag en
    /// de inputs van het besluit — staat in de melding niet, want dat is er al.
    #[error(
        "de bekendmaking van besluit '{besluit}' van cel '{cell}' kan niet draaien: de stage \
         wacht op {missing}"
    )]
    BekendmakingWachtOpInvoer {
        /// De cel die zou bekendmaken.
        cell: String,
        /// De besluit-definitie waarvan de bekendmaking gevraagd werd.
        besluit: String,
        /// De invoer waarop de stage wacht.
        missing: String,
    },

    /// Een verplichting wacht op de bekendmaking, maar die levert de uitkomst
    /// niet die haar vervaldag hoort te geven.
    ///
    /// `vanaf: bekendmaking` zegt dat de wet de vervaldag aan de bekendmaking
    /// hangt, en `vervaldatum` noemt de uitkomst die hem levert; wélke dag dat
    /// is, hoort dan óók uit de wet te komen. Levert de stage die uitkomst niet
    /// (of geen datum), dan zou het platform zelf een termijn moeten verzinnen,
    /// en dat is precies wat het niet doet.
    #[error(
        "de bekendmaking van besluit '{besluit}' van cel '{cell}' levert geen datum onder \
         '{veld}', terwijl er een verplichting op de bekendmaking wacht die daar haar \
         vervaldag uit haalt ({found})"
    )]
    BekendmakingZonderBetaaldatum {
        /// De cel die bekendmaakte.
        cell: String,
        /// De besluit-definitie waarvan de bekendmaking gevraagd werd.
        besluit: String,
        /// De uitkomst die de verplichting als vervaldatum noemt.
        veld: String,
        /// Wat de stage wél opleverde.
        found: String,
    },

    /// De procedure vraagt bij de bekendmaking geen datum.
    ///
    /// De dag van de bekendmaking is het `op_moment` van haar gram, en de
    /// procedure zegt onder welke naam die dag de stage in gaat: het eerste
    /// `requires`-veld van type `date`. Noemt ze er geen, dan is er geen naam om
    /// de dag onder aan te reiken, en een vaste naam van het platform zou de wet
    /// laten passen op de opstelling in plaats van andersom.
    #[error(
        "cel '{cell}', besluit '{besluit}': de stage BEKENDMAKING van procedure '{procedure}' \
         vraagt geen veld van type `date` in `requires`, dus er is geen naam waaronder de dag \
         van de bekendmaking aan de wet gegeven kan worden"
    )]
    BekendmakingZonderDatumveld {
        /// De cel die bekendmaakt.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// De procedure die voor het besluit geldt.
        procedure: String,
    },

    /// Het datumveld van de bekendmaking noemt een andere dag dan de klok.
    ///
    /// De dag van de bekendmaking is het `op_moment` van haar gram. Een andere
    /// dag invullen zou het gram iets laten zeggen wat het niet is: vroeger
    /// zou het beeld van een moment dat al voorbij is achteraf veranderen,
    /// later zou een gebeurtenis vastleggen die nog niet plaatsvond.
    #[error(
        "cel '{cell}', besluit '{besluit}': het veld '{veld}' is de dag van de bekendmaking en \
         hoort de stand van de klok te zijn ({clock}), niet '{value}'"
    )]
    BekendmakingDatumNietDeKlok {
        /// De cel die bekendmaakt.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// Het datumveld uit `requires`.
        veld: String,
        /// Wat er ingevuld werd.
        value: String,
        /// De stand van de klok.
        clock: String,
    },

    /// Een `requires`-veld van de bekendmaking heeft een type dat geen
    /// formulierveld kan zijn.
    #[error(
        "cel '{cell}', besluit '{besluit}': de stage BEKENDMAKING vraagt '{veld}' van type \
         '{type_name}', en dat kan een formulier niet invullen (wel: string, number, amount, \
         boolean, date)"
    )]
    BekendmakingVeldType {
        /// De cel die bekendmaakt.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// Het veld uit `requires`.
        veld: String,
        /// Het type dat de procedure noemt.
        type_name: String,
    },

    /// Een verplichting noemt haar vervaldatum niet, of niet zoals het hoort.
    ///
    /// `vanaf: bekendmaking` zonder `vervaldatum`, een `vervaldatum` die de
    /// bekendmaking niet kan opleveren, of een `vervaldatum` zonder `vanaf:
    /// bekendmaking`: alle drie een verplichting waarvan de eerste vervaldag
    /// stil zou wegvallen of stil genegeerd zou worden.
    #[error("cel '{cell}', besluit '{besluit}', verplichting in {origin}: {reason}")]
    ObligationVervaldatum {
        /// De cel met de besluit-definitie.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// Regeling, versie en artikel van de verplichting.
        origin: String,
        /// Wat eraan mankeert.
        reason: String,
    },

    /// Een `stage_uitkomsten`-uitkomst botst met wat het besluit of de hooks al
    /// leveren.
    ///
    /// Een uitkomst in het gram van het besluit wordt bij een latere stage niet
    /// herberekend en niet overschreven; een uitkomst die een hook ook levert,
    /// zou in het stage-gram twee herkomsten hebben.
    #[error("cel '{cell}', besluit '{besluit}': stage-uitkomst '{output}' {reason}")]
    StageUitkomst {
        /// De cel met de besluit-definitie.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
        /// De uitkomst uit `stage_uitkomsten`.
        output: String,
        /// Waarmee ze botst.
        reason: String,
    },

    /// Het decretogram draagt een wachtende verplichting die niet te lezen is.
    ///
    /// Onbereikbaar zolang alleen deze crate in [`crate::BESCHIKKINGEN`] schrijft:
    /// wat het besluit erin zette, leest de bekendmaking er weer uit. Het staat er
    /// omdat de helft inroosteren erger zou zijn dan niets: een termijn zonder
    /// bedrag of zonder partij is een belofte die niemand kan nakomen.
    #[error(
        "het besluit '{besluit}' van cel '{cell}' draagt een wachtende verplichting die niet \
         te lezen is"
    )]
    OnleesbareWachtendeVerplichting {
        /// De cel die besloot.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
    },

    /// Een actie legt vast in een stroom die dat niet kan dragen.
    ///
    /// Blijkt bij het optuigen en niet bij de eerste aanroep: een actie noemt haar
    /// formulier en haar stroom, dus of het gram dat eruit komt ooit kan landen,
    /// staat dan al vast.
    #[error(
        "actie '{action}': cel '{cell}' kan geen feit vastleggen in kroniekstroom \
         '{stream}': {reason}"
    )]
    ActionRecording {
        /// De actie uit het wereldbestand.
        action: String,
        /// De cel waarin vastgelegd zou worden.
        cell: String,
        /// De stroom die het niet kan dragen.
        stream: String,
        /// Waarom niet, in de woorden van de cel.
        reason: String,
    },

    /// Een actie levert een feit aan de cel die het zelf vastlegt.
    ///
    /// Dan zou hetzelfde gram twee keer in dezelfde kroniek landen. Een levering
    /// gaat van de ene organisatie naar de andere; aan jezelf leveren wat je net
    /// zelf vastlegde, is geen tweede feit.
    #[error("actie '{action}' levert aan cel '{cell}', maar die legt het feit zelf al vast")]
    DeliveryToSelf {
        /// De actie uit het wereldbestand.
        action: String,
        /// De cel die beide kanten zou zijn.
        cell: String,
    },

    /// Er is een instelling gewijzigd die het wereldbestand niet kent.
    ///
    /// Een instelling bijzetten die nergens gebruikt wordt, zou een knop zijn die
    /// niets doet; een typfout in een naam zou er precies zo uitzien.
    #[error("de wereld kent geen instelling '{setting}' (wel: {known})")]
    UnknownWorldSetting {
        /// De naam die gewijzigd zou worden.
        setting: String,
        /// Komma-gescheiden lijst van de instellingen die er wél zijn.
        known: String,
    },

    /// Een instelling die al door een besluit gebruikt is, wordt gewijzigd.
    ///
    /// Een besluit legt vast waarop besloten is. Een instelling die eronder
    /// vandaan geschoven wordt, laat het gram iets anders zeggen dan er gebeurd
    /// is — en precies dat is wat een decretogram onmogelijk hoort te maken.
    #[error(
        "instelling '{setting}' is al gebruikt door besluit '{besluit}' van cel '{cell}' \
         en staat daarmee vast; wie haar wil wijzigen, begint een nieuwe wereld"
    )]
    SettingInUse {
        /// De instelling die vast staat.
        setting: String,
        /// De cel die het besluit nam.
        cell: String,
        /// Het besluit dat haar gebruikte.
        besluit: String,
    },

    /// Het scenariobestand kon niet gelezen worden.
    #[error("kon scenario '{path}' niet lezen: {source}")]
    ScenarioRead {
        /// Het pad dat gelezen werd.
        path: PathBuf,
        /// De onderliggende I/O-fout.
        source: std::io::Error,
    },

    /// Een YAML-bestand kon niet gelezen worden.
    #[error("kon '{path}' niet lezen: {source}")]
    FileRead {
        /// Het pad dat gelezen werd.
        path: PathBuf,
        /// De onderliggende I/O-fout.
        source: std::io::Error,
    },

    /// Het scenario is geen geldige YAML of mist verplichte velden.
    #[error("kon scenario niet lezen: {0}")]
    ScenarioParse(#[from] serde_yaml_ng::Error),

    /// Een regelingdocument uit het corpus is geen geldige YAML of mist `$id`.
    ///
    /// Eigen variant, want zonder pad wijst de melding de lezer naar het
    /// scenariobestand terwijl het bestand in het corpus stuk is.
    #[error("kon regelingdocument '{path}' niet lezen: {source}")]
    RegulationParse {
        /// Het gelezen bestand.
        path: PathBuf,
        /// De onderliggende YAML-fout.
        source: serde_yaml_ng::Error,
    },

    /// De engine kon een regeling niet laden of uitvoeren.
    #[error("engine: {0}")]
    Engine(#[from] regelrecht_engine::EngineError),
}

/// Resultaattype van de simulator.
pub type Result<T> = std::result::Result<T, SimulatorError>;
