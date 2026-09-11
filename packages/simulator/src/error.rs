//! Fouten van de simulator.
//!
//! De teksten zijn Nederlands, net als de rest van de gebruikersgerichte
//! uitvoer in deze repo; identifiers blijven Engels.

use std::path::PathBuf;

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
    #[error("lexostatus '{cell}.{lexostatus}' vereist parameter '{parameter}'")]
    MissingParameter {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// De gevraagde lexostatus.
        lexostatus: String,
        /// De ontbrekende parameter.
        parameter: String,
    },

    /// De vraag bevat een parameter die de lexostatus niet documenteert.
    #[error(
        "lexostatus '{cell}.{lexostatus}' kent geen parameter '{parameter}' \
         (gedocumenteerd: {documented})"
    )]
    UndocumentedParameter {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// De gevraagde lexostatus.
        lexostatus: String,
        /// De onbekende parameter.
        parameter: String,
        /// Komma-gescheiden lijst van gedocumenteerde parameters.
        documented: String,
    },

    /// Een parameter heeft een ander type dan gedocumenteerd.
    #[error(
        "parameter '{parameter}' van lexostatus '{cell}.{lexostatus}' is {actual}, \
         verwacht {expected}"
    )]
    ParameterType {
        /// Cel waaraan gevraagd werd.
        cell: String,
        /// De gevraagde lexostatus.
        lexostatus: String,
        /// De parameter met het verkeerde type.
        parameter: String,
        /// Het gedocumenteerde type.
        expected: &'static str,
        /// Het aangeboden type.
        actual: &'static str,
    },

    /// De reductie verwijst met `$naam` naar een niet-gedocumenteerde parameter.
    #[error(
        "cel '{cell}': reductie van '{lexostatus}' verwijst naar '${reference}', \
         maar dat is geen gedocumenteerde parameter"
    )]
    UnknownReference {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus met de kapotte verwijzing.
        lexostatus: String,
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

    /// Een reductie grijpt naar een regeling die deze cel niet zelf geladen heeft.
    ///
    /// Een reductie raakt uitsluitend de eigen feiten van de cel; combineren over
    /// cellen heen is synthese en hoort bij een consument (RFC-022 §4.1).
    #[error(
        "cel '{cell}': reductie van '{lexostatus}' gebruikt regeling '{regulation}', \
         die deze cel niet zelf geladen heeft"
    )]
    ForeignRegulation {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus met de vreemde regeling.
        lexostatus: String,
        /// De regeling waarnaar verwezen wordt.
        regulation: String,
    },

    /// Een lexostatus publiceert een uitkomst die haar bron niet kent.
    ///
    /// Wat een cel publiceert is een belofte aan de consument; een naam die
    /// geen enkele geladen versie van de regeling oplevert — of die in geen
    /// enkele vastlegging van de kroniekstroom voorkomt — kan die belofte niet
    /// waarmaken. Dat blijkt bij het optuigen, net als bij
    /// [`SimulatorError::ForeignRegulation`], en niet pas bij de eerste vraag.
    #[error(
        "cel '{cell}': lexostatus '{lexostatus}' publiceert uitkomst '{output}', \
         maar {origin} kent die niet (wel: {known})"
    )]
    UnknownOutput {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus met de onbekende uitkomst.
        lexostatus: String,
        /// Waar de uitkomst uit had moeten komen: `regeling '…'` of
        /// `kroniekstroom '…'`. Niet `source`: dat veld zou thiserror als de
        /// onderliggende fout lezen.
        origin: String,
        /// De uitkomstnaam die niet bestaat.
        output: String,
        /// Komma-gescheiden lijst van namen die de bron wél kent.
        known: String,
    },

    /// Een kroniekfilter verwijst naar een stroom die de cel niet houdt.
    ///
    /// Een reductie raakt uitsluitend de eigen feiten van de cel. Een onbekende
    /// stroomnaam is de kroniek-tegenhanger van
    /// [`SimulatorError::ForeignRegulation`] en blijkt op hetzelfde moment.
    #[error(
        "cel '{cell}': kroniekfilter van '{lexostatus}' verwijst naar kroniekstroom \
         '{stream}', die deze cel niet houdt (wel: {known})"
    )]
    UnknownStream {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus met de onbekende stroom.
        lexostatus: String,
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

    /// Een kroniekfilter sleutelt of filtert op een veld dat de stroom niet kent.
    ///
    /// Zo'n filter zou stil "niets vastgesteld" antwoorden op elke vraag, en dat
    /// is niet te onderscheiden van een leeg verleden.
    #[error(
        "cel '{cell}': kroniekfilter van '{lexostatus}' gebruikt veld '{field}', \
         maar kroniekstroom '{stream}' kent dat niet (wel: {known})"
    )]
    UnknownFilterField {
        /// Cel waarin de definitie staat.
        cell: String,
        /// De lexostatus met het onbekende veld.
        lexostatus: String,
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
    UnknownStream {
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

    /// Er wordt gereduceerd over een moment dat nog niet gebeurd is.
    ///
    /// De wereld weet niet wat er na haar klok gebeurt: triggers die nog moeten
    /// afgaan hebben niets vastgelegd. Een antwoord "op" zo'n moment zou een
    /// voorspelling zijn die zich voordoet als een reductie.
    #[error(
        "cel '{cell}': reductie van '{lexostatus}' op {op_moment} vraagt een moment ná de klok \
         van de wereld ({clock})"
    )]
    MomentAfterClock {
        /// De bevraagde cel.
        cell: String,
        /// De gevraagde lexostatus.
        lexostatus: String,
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
