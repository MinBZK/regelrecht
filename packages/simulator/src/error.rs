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
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Lexostatus => "lexostatus",
            Self::Besluit => "besluit",
            Self::Actie => "actie",
            Self::Termijn => "termijn",
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
        "cel '{cell}': verplichting van besluit '{besluit}' noemt bedrag '{amount}'; \
         dat moet een uitkomst van dit besluit zijn, als $naam (uitkomsten: {outputs})"
    )]
    ObligationAmount {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// Wat er als bedrag stond.
        amount: String,
        /// Komma-gescheiden lijst van de uitkomsten die het besluit vastlegt.
        outputs: String,
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

    /// Een verplichting verwijst naar een instelling die het wereldbestand niet
    /// heeft.
    #[error(
        "cel '{cell}': verplichting van besluit '{besluit}' verwijst naar instelling \
         '{setting}', maar die staat niet in `settings` van het wereldbestand \
         (wel: {known})"
    )]
    UnknownSetting {
        /// Cel waarin de definitie staat.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// De instelling waarnaar verwezen wordt.
        setting: String,
        /// Komma-gescheiden lijst van de instellingen die er wél zijn.
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

    /// Een verplichting zou vervallen vóór het besluit dat haar schept.
    ///
    /// Een termijn met een datum in het verleden zou bij het nakomen een
    /// vastlegging op dat eerdere moment opleveren, en dan verandert het beeld van
    /// toen doordat de wereld verder loopt — precies wat een kroniek niet doet.
    #[error(
        "cel '{cell}': verplichting van besluit '{besluit}' vervalt op {from}, \
         vóór het besluit van {op_moment}; een verplichting kan niet vervallen \
         vóór het besluit waaruit ze volgt"
    )]
    ObligationBeforeDecision {
        /// Cel die besluit.
        cell: String,
        /// Het besluit met de verplichting.
        besluit: String,
        /// De uitgerekende startdatum.
        from: String,
        /// Het moment van het besluit.
        op_moment: String,
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
