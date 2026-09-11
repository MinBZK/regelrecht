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
use regelrecht_simulator::SimulatorError;
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
    /// * **500** — al het andere. Een wereld die niet opgetuigd kan worden, een
    ///   regeling die niet gelezen kan worden, een definitie die niet klopt:
    ///   dat zijn eigenschappen van het wereldbestand waarmee dit proces
    ///   gestart is, en niet van het verzoek dat het net binnenkreeg.
    pub fn from_simulator(error: &SimulatorError) -> Self {
        use SimulatorError as E;
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
            | E::UnknownWorldSetting { .. } => StatusCode::BAD_REQUEST,

            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: error.to_string(),
        }
    }
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
}
