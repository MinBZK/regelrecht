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

    /// Twee cellen met hetzelfde id in één scenario.
    #[error("scenario definieert cel '{cell}' twee keer")]
    DuplicateCell {
        /// Het dubbele cel-id.
        cell: String,
    },

    /// Een vraag richt zich tot een cel die het scenario niet kent.
    #[error("scenario kent geen cel '{cell}'")]
    UnknownCell {
        /// Het onbekende cel-id.
        cell: String,
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

    /// De engine kon een regeling niet laden of uitvoeren.
    #[error("engine: {0}")]
    Engine(#[from] regelrecht_engine::EngineError),
}

/// Resultaattype van de simulator.
pub type Result<T> = std::result::Result<T, SimulatorError>;
