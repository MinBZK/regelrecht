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
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Lexostatus => "lexostatus",
            Self::Besluit => "besluit",
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
