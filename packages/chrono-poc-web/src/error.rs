//! De fout die deze laag naar buiten geeft: een status en een Nederlandse
//! melding, als JSON.
//!
//! Eén type voor alles wat er mis kan gaan, want er is maar één soort
//! consument: een frontend die de melding aan een mens laat zien. De melding is
//! Nederlands omdat de simulator dat is — de uitleg die een cel geeft over een
//! actie die nog niet kan, of een parameter die niet gedocumenteerd is, is wat
//! hier doorkomt, en die zou door hertalen alleen maar armer worden.
//!
//! De **status** komt uit de foutvariant van de simulator en niet uit de plek
//! waar hij vandaan kwam. Een onbekende cel is een 404 of hij opgevraagd wordt
//! via `/api/cells/...` of langs een actie; een wereld die bij het optuigen
//! omvalt is een 500, ook al deed de aanroeper niets fout. Zie
//! [`ApiError::from_simulator`] voor de hele afbeelding.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use regelrecht_simulator::{EngineError, SimulatorError};
use serde::Serialize;

/// Een mislukt verzoek: een HTTP-status plus wat er aan de hand is.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

/// Het lichaam van een fout. Één veld, altijd hetzelfde veld: een client die
/// per status een andere vorm moet uitpakken, pakt er één verkeerd uit.
#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
}

impl ApiError {
    /// Het verzoek zelf klopt niet: een onleesbare datum, een parameter die
    /// deze lexostatus niet documenteert.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    /// Er bestaat niet wat er gevraagd wordt: een cel, een lexostatus, een
    /// actie.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    /// Onze fout. De melding gaat mee naar de client omdat er aan de andere
    /// kant van deze PoC een ontwikkelaar zit en geen burger; er zit geen
    /// gegeven van een ander in een simulatorfout dat een ander niet mag zien.
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    /// De status waarmee deze fout naar buiten gaat.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// De melding waarmee deze fout naar buiten gaat.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Een fout van de simulator, met de status die bij de variant hoort.
    ///
    /// Drie groepen, en de rest is onze schuld:
    ///
    /// * **404** — wat het pad aanwijst bestaat niet: een cel, een lexostatus,
    ///   een actie. De simulator zet in zijn melding welke namen er wél zijn, en
    ///   die hoort een client te zien.
    /// * **409** — het bestaat, maar de wereld staat er nu niet naar. Dit is
    ///   geen verkeerd verzoek en geen defect: het verhaal is er nog niet.
    /// * **400** — het verzoek klopt niet tegen wat de definitie belooft: een
    ///   parameter of instelling te veel, te weinig, of van het verkeerde type.
    ///   Ook een engine-fout die over een aangeleverde waarde gaat hoort hier:
    ///   zie [`engine_input_error`].
    /// * **500** — al het andere. Een wereld die niet opgetuigd kan worden, een
    ///   regeling die niet gelezen kan worden, een definitie die niet klopt:
    ///   dat zijn eigenschappen van het wereldbestand waarmee dit proces
    ///   gestart is, en niet van het verzoek dat het net binnenkreeg.
    pub fn from_simulator(error: &SimulatorError) -> Self {
        use SimulatorError as E;

        if let E::Engine(engine) = error {
            if let Some(message) = engine_input_error(engine) {
                return Self::bad_request(message);
            }
        }

        let status = match error {
            E::UnknownCell { .. } | E::UnknownLexostatus { .. } | E::UnknownAction { .. } => {
                StatusCode::NOT_FOUND
            }

            E::ActionNotAvailable { .. }
            | E::SettingInUse { .. }
            | E::ClockRunsBackwards { .. }
            | E::MomentAfterClock { .. } => StatusCode::CONFLICT,

            E::MissingParameter { .. }
            | E::UndocumentedParameter { .. }
            | E::ParameterType { .. }
            | E::ParameterDate { .. }
            | E::UnknownWorldSetting { .. } => StatusCode::BAD_REQUEST,

            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: error.to_string(),
        }
    }
}

/// De Nederlandse melding als deze engine-fout over een **aangeleverde waarde**
/// gaat, en niets als het een fout in de regeling of in de opstelling is.
///
/// De gedocumenteerde parameters vangen het meeste al af: wat als `date`
/// gedocumenteerd staat, wordt bij het binden getoetst en komt hier nooit meer
/// langs. Wat overblijft is de waarde die pas ín een regeling een datum blijkt
/// te moeten zijn, of de parameter die de regeling leeg vindt. Zonder deze
/// vertaling gaat dat als 500 naar buiten met de engine-tekst erin — "Failed to
/// parse date '01-12-2026': trailing input" — en dat is twee keer verkeerd: het
/// zegt "wij zijn stuk" waar een veld verkeerd staat, en het zegt het in het
/// Engels van een andere laag.
fn engine_input_error(error: &EngineError) -> Option<String> {
    match error {
        // De traced-variant is een omhulsel om de echte fout; wat erin zit
        // bepaalt waar dit verzoek aan toe is.
        EngineError::TracedError { source, .. } => engine_input_error(source),

        EngineError::InvalidDate(detail) => Some(format!(
            "een datum is niet te lezen ({detail}); een datum hoort als jjjj-mm-dd aangeleverd \
             te worden"
        )),

        EngineError::MissingParameter { name, .. } => Some(format!(
            "parameter '{name}' heeft geen waarde; de regeling kan zo niet uitgevoerd worden"
        )),

        EngineError::InvalidOperation(message) => unreadable_date(message).map(|raw| {
            format!("'{raw}' is geen datum; een datum hoort als jjjj-mm-dd aangeleverd te worden")
        }),

        _ => None,
    }
}

/// Waar de engine een datum-parse-fout mee opent.
const ENGINE_DATE_FAILURE: &str = "Failed to parse date '";

/// De waarde uit een datum-parse-fout van de engine, als het er een is.
///
/// De engine heeft voor "deze tekst is geen datum" geen eigen foutvariant — ze
/// zet het als `InvalidOperation` neer, met de waarde in de melding. Zolang dat
/// zo is, is dit de enige manier om juist die fout te herkennen, en dat is de
/// moeite waard: het is de fout die een mens maakt door de Nederlandse notatie
/// te typen in een veld dat ISO verwacht. Herkent dit de melding niet, dan gaat
/// de fout als 500 naar buiten: minder behulpzaam, maar niet onwaar.
///
/// Eén vorm dus, en met opzet niet meer. De engine wijst daarnaast een
/// niet-canonieke datum af ("Date '…' is not in canonical …"), maar die komt
/// hier via een `date`-parameter niet langs — die strandt al bij het binden, met
/// de melding van de cel zelf. Een tweede prefix zou dus een tweede breekbare
/// tekstafspraak zijn voor een geval dat deze laag niet ziet.
/// `een_andere_invalid_operation_wordt_geen_datumfout` pint vast dat hij als 500
/// naar buiten gaat, zodat dat een keuze blijft en geen vergissing wordt.
fn unreadable_date(message: &str) -> Option<&str> {
    message
        .strip_prefix(ENGINE_DATE_FAILURE)
        .and_then(|rest| rest.split('\'').next())
        // Een melding die op de aanhef ophoudt draagt geen waarde om te noemen,
        // en "'' is geen datum" helpt niemand verder.
        .filter(|raw| !raw.is_empty())
}

impl From<SimulatorError> for ApiError {
    fn from(error: SimulatorError) -> Self {
        Self::from_simulator(&error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Een 500 hoort in het log te staan ook als de client hem wegklikt; de
        // andere statussen zijn antwoorden en geen storingen.
        if self.status.is_server_error() {
            tracing::error!(error = %self.message, "verzoek mislukt");
        }
        (
            self.status,
            Json(ErrorBody {
                error: &self.message,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// De drie groepen uit de doc-comment, vastgepind op een echte fout per
    /// groep. Zonder dit is de afbeelding proza: een variant die van groep
    /// wisselt verandert dan stil een 409 in een 500 (of erger, andersom).
    #[test]
    fn de_statusafbeelding_volgt_de_soort_fout() {
        let onbekende_cel = SimulatorError::UnknownCell {
            cell: "kiesraad".to_string(),
        };
        assert_eq!(
            ApiError::from_simulator(&onbekende_cel).status(),
            StatusCode::NOT_FOUND
        );

        let kan_nu_niet = SimulatorError::ActionNotAvailable {
            action: "toeslagen.toekenning".to_string(),
            reason: "er ligt nog geen aanvraag".to_string(),
        };
        let conflict = ApiError::from_simulator(&kan_nu_niet);
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert!(
            conflict.message().contains("er ligt nog geen aanvraag"),
            "de uitleg van de simulator hoort door te komen, kreeg {}",
            conflict.message()
        );

        let verkeerd_type = SimulatorError::ParameterType {
            cell: "brp".to_string(),
            subject: regelrecht_simulator::Subject::Lexostatus,
            name: "partnerschap".to_string(),
            parameter: "bsn".to_string(),
            expected: "string",
            actual: "number",
        };
        assert_eq!(
            ApiError::from_simulator(&verkeerd_type).status(),
            StatusCode::BAD_REQUEST
        );

        let gebroken_wereld = SimulatorError::RegulationNotFound {
            regulation: "wet_op_de_zorgtoeslag".to_string(),
            root: std::path::PathBuf::from("/corpus"),
        };
        assert_eq!(
            ApiError::from_simulator(&gebroken_wereld).status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    /// Een datum die niet te lezen is, is een fout in het verzoek en niet in de
    /// server — ook als de engine hem pas onderweg tegenkomt. De melding noemt
    /// de waarde en de notatie die wél gelezen wordt, in het Nederlands.
    #[test]
    fn een_onleesbare_datum_uit_de_engine_is_een_verzoekfout() {
        let geen_datum = SimulatorError::Engine(EngineError::InvalidOperation(
            "Failed to parse date '01-12-2026': trailing input. Expected format: YYYY-MM-DD"
                .to_string(),
        ));
        let fout = ApiError::from_simulator(&geen_datum);
        assert_eq!(fout.status(), StatusCode::BAD_REQUEST);
        assert!(fout.message().contains("01-12-2026"), "{}", fout.message());
        assert!(fout.message().contains("jjjj-mm-dd"), "{}", fout.message());
        assert!(
            !fout.message().contains("Failed to parse"),
            "de engine-tekst hoort niet door te komen: {}",
            fout.message()
        );
    }

    /// Een parameter die de regeling leeg vindt, is evengoed iets wat de
    /// aanroeper kan herstellen.
    #[test]
    fn een_lege_parameter_uit_de_engine_is_een_verzoekfout() {
        let leeg = SimulatorError::Engine(EngineError::MissingParameter {
            law_id: "wet_op_de_zorgtoeslag".to_string(),
            name: "bsn".to_string(),
            value: "null".to_string(),
        });
        let fout = ApiError::from_simulator(&leeg);
        assert_eq!(fout.status(), StatusCode::BAD_REQUEST);
        assert!(fout.message().contains("bsn"), "{}", fout.message());
    }

    /// En andersom: een engine-fout die niets met de aangeleverde waarden te
    /// maken heeft, blijft een 500. Zou alles van de engine een 400 worden, dan
    /// zou een kapotte regeling de aanroeper de schuld geven.
    #[test]
    fn een_engine_fout_over_de_regeling_blijft_onze_schuld() {
        let kapotte_wet = SimulatorError::Engine(EngineError::LawNotFound(
            "wet_op_de_zorgtoeslag".to_string(),
        ));
        assert_eq!(
            ApiError::from_simulator(&kapotte_wet).status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    /// `InvalidOperation` is de variant waarin de engine haar datum-leesfout
    /// stopt, maar ook van alles daarbuiten. Alleen de eerste soort wordt een
    /// 400; de rest hoort onze schuld te blijven, met de melding van de engine
    /// erbij in plaats van een verzonnen uitleg over datums.
    ///
    /// Dit pint de andere kant van de prefixherkenning vast: gaat de engine haar
    /// melding anders schrijven, dan valt hier niets stil om — alles wordt dan
    /// weer een 500, en dát is wat deze test bewaakt.
    #[test]
    fn een_andere_invalid_operation_wordt_geen_datumfout() {
        for melding in [
            "Unknown operation 'FROBNICATE'",
            // Dezelfde functie in de engine, een paar regels verderop: dit is
            // hoe ze een niet-canonieke datum afwijst. Een `date`-parameter komt
            // hier niet meer langs (die strandt bij het binden); een waarde die
            // pas ín een regeling een datum blijkt te zijn wel, en die valt
            // bewust in de 500 in plaats van in een geraden vertaling.
            "Date '2026-1-1' is not in canonical YYYY-MM-DD form (use zero-padded \
             components, e.g. '2026-01-01')",
        ] {
            let fout = ApiError::from_simulator(&SimulatorError::Engine(
                EngineError::InvalidOperation(melding.to_string()),
            ));
            assert_eq!(
                fout.status(),
                StatusCode::INTERNAL_SERVER_ERROR,
                "'{melding}' is geen datum-leesfout"
            );
            assert!(
                !fout.message().contains("jjjj-mm-dd"),
                "hier hoort geen uitleg over datumnotatie te staan: {}",
                fout.message()
            );
        }
    }
}
