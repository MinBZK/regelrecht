//! De routes van een cel en van een proces. De runtime biedt ze aan onder
//! `/cellen/<id>` en `/processen/<id>` (zie [`crate::runtime`]).
//!
//! Een cel legt vast, bewaart en reduceert ([`cel`]). Elke cel:
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/kroniek` | de grammen, elk met YAML |
//! | `GET /api/zaken/{zaakkenmerk}` | de grammen van een zaak, elk met YAML; 404 als de cel de zaak niet kent |
//! | `GET /api/lexostatus/{naam}?<input>=...` | een reductie, met de inputs als query |
//! | `POST /api/lexostatus/{naam}/proef` | `{concept, inputs}`: bouwt het gram in het geheugen en reduceert de kroniek mét dat gram; legt niets vast |
//! | `POST /api/grammen` | `{actor, stroom, event, intake, external, zaakkenmerk?, besluit?}`: bouwt het gram, valideert het, controleert de actor en de zaak en legt het vast |
//! | `GET /api/stroom` | de stroomdefinities van de cel, met hun hash |
//!
//! Een proces handelt: het informeert, concludeert en laat een cel
//! vastleggen ([`proces`]). Elk proces:
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/voorbeelden` | standaardgegevens per handeling (`voorbeelden` in `proces.yaml`), ook zonder login |
//!
//! Een proces met een portaal (rol aanvrager, nep-eHerkenning) heeft daarnaast
//! ([`sessie`], [`portaal`]):
//!
//! | Route | Doet |
//! |---|---|
//! | `POST /api/eherkenning/login` | `{kvk, persoon}` naar een sessie |
//! | `GET /api/eherkenning/sessie` | wie is ingelogd |
//! | `POST /api/eherkenning/logout` | sessie beeindigen |
//! | `GET /api/formulier` | de velden van het aanvraagformulier, uit de stroom van de cel |
//! | `POST /api/aanvraag/toets` | proefreductie in de cel, synthese, synthese per regel, engine |
//! | `POST /api/aanvraag` | de cel legt het gram vast |
//! | `GET /api/mogelijkheden` | wat het portaal aanbiedt volgens het beleid, per tijdvak, met trace |
//!
//! Een proces met de rol behandelaar (nagebootste medewerkerslogin) heeft:
//!
//! | Route | Doet |
//! |---|---|
//! | `POST /api/medewerker/login` | `{naam}` naar een sessie |
//! | `GET /api/medewerker/sessie` | wie is ingelogd |
//! | `POST /api/medewerker/logout` | sessie beeindigen |
//!
//! en met een `behandeling`, alleen voor de behandelaar ([`behandeling`]):
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/werkvoorraad` | de lijst-lexostatus van de werkvoorraad, uit de cel |
//! | `GET /api/zaken/{zaakkenmerk}` | de grammen van de zaak, het besluitformulier en een proefbesluit zonder oordelen |
//! | `POST /api/zaken/{zaakkenmerk}/proefbesluit` | `{formulier}` naar een proefbesluit; niets wordt vastgelegd |
//! | `POST /api/zaken/{zaakkenmerk}/besluit` | het besluit nemen; de cel legt het vast |
//!
//! Tussen proces en cel is geen beveiligingscontext: de routes van een cel
//! vragen geen login, net als een bron van een andere organisatie.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, FixedOffset};
use serde_json::json;

use crate::transport::TransportFout;

pub mod behandeling;
pub mod cel;
pub mod portaal;
pub mod proces;
pub mod sessie;

pub use cel::{als_yaml, cel_beschrijving, cel_router, CelState};
pub use proces::{proces_beschrijving, proces_router, ProcesState};

/// Levert het moment waarop iets tot feit wordt gemaakt.
pub type Klok = Arc<dyn Fn() -> DateTime<FixedOffset> + Send + Sync>;

/// De klok van de runtime: nu, in Nederlandse tijd.
pub fn systeemklok() -> Klok {
    Arc::new(|| {
        chrono::Utc::now()
            .with_timezone(&chrono_tz::Europe::Amsterdam)
            .fixed_offset()
    })
}

/// Een fout als `{"fout": "..."}` met een status.
pub struct Fout(StatusCode, String);

impl IntoResponse for Fout {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"fout": self.1}))).into_response()
    }
}

fn fout(status: StatusCode, tekst: impl Into<String>) -> Fout {
    Fout(status, tekst.into())
}

/// Een fout van de runtime zelf (500).
fn intern(tekst: impl Into<String>) -> Fout {
    fout(StatusCode::INTERNAL_SERVER_ERROR, tekst)
}

/// Een fout van de cel als antwoord van het proces: dezelfde status en
/// dezelfde tekst. Een cel die niet antwoordt, of onleesbaar, is een fout van
/// de runtime.
fn van_cel(f: TransportFout) -> Fout {
    match f {
        TransportFout::Antwoord {
            status,
            fout: tekst,
        } => Fout(
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            tekst,
        ),
        TransportFout::Onbereikbaar(r) => intern(format!("de cel is onbereikbaar: {r}")),
        TransportFout::Json(r) => intern(format!("de cel antwoordde onleesbaar: {r}")),
    }
}
