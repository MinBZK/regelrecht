//! De routes van een cel en van een proces. De runtime biedt ze aan onder
//! `/cellen/<id>` en `/processen/<id>` (zie [`crate::runtime`]).
//!
//! Een cel legt vast, bewaart en reduceert ([`cel`]). Elke cel:
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/kroniek` | met het runtime- of leestoken: de grammen, elk met YAML |
//! | `GET /api/zaken/{zaakkenmerk}` | met het runtime- of leestoken: de grammen van een zaak, elk met YAML; 404 als de cel de zaak niet kent |
//! | `GET /api/lexostatus/{naam}?<input>=...` | met het runtime- of leestoken: een reductie, met de inputs als query; `zaakstand` biedt de runtime aan voor elke cel met een zaak |
//! | `POST /api/lexostatus/{naam}/proef` | alleen met het runtime-token: `{concept, inputs}`: bouwt het gram in het geheugen en reduceert de kroniek mét dat gram; legt niets vast |
//! | `POST /api/grammen` | alleen met het runtime-token: `{actor, stroom, event, intake, external, zaakkenmerk?, besluit?, zaak_grammen?}`: bouwt het gram, valideert het, controleert de actor en de zaak en legt het vast |
//! | `GET /api/stroom` | de stroomdefinities van de cel, met hun hash |
//!
//! Een proces handelt: het informeert, concludeert en laat een cel
//! vastleggen ([`proces`]). Elk proces:
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/voorbeelden` | standaardgegevens per handeling (`voorbeelden` in `proces.yaml`), ook zonder login |
//!
//! Een proces met rollen heeft de routes van zijn kanalen ([`sessie`]); elk
//! kanaal is nagebootst en staat in `kanalen` in `proces.yaml`:
//!
//! | Route | Doet |
//! |---|---|
//! | `POST /api/kanalen/{kanaal}/login` | de velden van het kanaal (en `rol` als er langs het kanaal meer dan een rol inlogt) naar een sessie |
//! | `GET /api/kanalen/{kanaal}/sessie` | wie langs dit kanaal is ingelogd |
//! | `POST /api/kanalen/{kanaal}/logout` | sessie beeindigen |
//! | `GET /api/sessie` | wie er is ingelogd, langs welk kanaal ook |
//!
//! Elke andere route hoort bij een routegroep; een rol noemt de groepen die
//! ze mag gebruiken (`rollen.<rol>.routes`). Een proces met een portaal, voor
//! een rol met routes `portaal` ([`portaal`]):
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/formulier` | de velden van het aanvraagformulier, uit de stroom van de cel |
//! | `POST /api/aanvraag/toets` | proefreductie in de cel, synthese, synthese per regel, engine |
//! | `POST /api/aanvraag` | de cel legt het gram vast |
//! | `GET /api/mogelijkheden` | wat het portaal aanbiedt volgens het beleid, per tijdvak, met trace |
//!
//! Met een rol met routes `loket` ([`loket`]):
//!
//! | Route | Doet |
//! |---|---|
//! | `POST /api/loket/aanvraag` | `{aanvrager, ontvangen_op, external}`: een aanvraag die langs een andere weg binnenkwam, met de dag van ontvangst |
//!
//! En met een `behandeling`, voor een rol met routes `behandeling`
//! ([`behandeling`]):
//!
//! | Route | Doet |
//! |---|---|
//! | `GET /api/werkvoorraad` | de lijst-lexostatus van de werkvoorraad, uit de cel |
//! | `GET /api/inzage/{cel}/kroniek` | de kroniek van een cel die het proces leest ([`inzage`]) |
//! | `GET /api/inzage/{cel}/lexostatus/{naam}?...` | een lexostatus van zo'n cel |
//! | `GET /api/zaken/{zaakkenmerk}` | de grammen van de zaak, de procedure, de rechtsbescherming, en per handeling haar formulier, of zij kan, en een proef zonder formulier |
//! | `POST /api/zaken/{zaakkenmerk}/handelingen/{naam}/proef` | `{formulier}` naar een handeling op proef; niets wordt vastgelegd |
//! | `POST /api/zaken/{zaakkenmerk}/handelingen/{naam}` | `{formulier, gebeurd?}`: de handeling nemen, of een gebeurd feit melden; de cel legt haar vast |
//!
//! Tussen proces en cel is geen beveiligingscontext. Vastleggen en op proef
//! reduceren mag alleen een proces van deze runtime: het interne transport
//! stuurt het runtime-token mee ([`crate::transport::RuntimeToken`]); zonder
//! token 401, met een ander 403. Lezen (kroniek, zaak, lexostatus) vraagt
//! datzelfde token of het leestoken dat runtimes delen die elkaar mogen
//! lezen (`CEL_LEES_TOKEN`), want een gram draagt de identiteit en de
//! intake van wie indiende. Alleen de stroomdefinities zijn open.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, FixedOffset};
use serde_json::json;

use crate::transport::TransportFout;

pub mod behandeling;
pub mod cel;
pub mod inzage;
pub mod loket;
pub mod portaal;
pub mod proces;
pub mod sessie;

pub use cel::{als_yaml, cel_beschrijving, cel_router, CelState};
pub use proces::{proces_beschrijving, proces_router, HandelingState, ProcesState};

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
